//! Regex execution shared by VRL's public regex operators. The Rust parser and
//! metadata remain authoritative; PCRE2 JIT is an explicitly selected backend.
use super::regex_capture::{CaptureGroups, CaptureView, LocatedCaptures};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    fmt,
    ops::{Deref, Index},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use thread_local::ThreadLocal;

#[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
#[path = "regex_pcre2.rs"]
mod pcre2;
#[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
#[path = "regex_translate.rs"]
mod translate;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RegexEngine {
    #[default]
    Rust,
    Pcre2Jit,
}

thread_local! { static ACTIVE_ENGINE: Cell<RegexEngine> = const { Cell::new(RegexEngine::Rust) }; }
#[cfg(test)]
thread_local! { static JIT_EXECUTIONS: Cell<u64> = const { Cell::new(0) }; }
#[cfg(all(test, feature = "pcre2-jit", not(target_arch = "wasm32")))]
fn record_jit() {
    JIT_EXECUTIONS.with(|count| count.set(count.get() + 1));
}

// The guard restores a thread-local setting and must stay on its creating thread.
pub struct RegexEngineGuard(RegexEngine, std::marker::PhantomData<std::rc::Rc<()>>);
impl Drop for RegexEngineGuard {
    fn drop(&mut self) {
        ACTIVE_ENGINE.with(|slot| slot.set(self.0));
    }
}
impl RegexEngine {
    pub fn current() -> Self {
        ACTIVE_ENGINE.with(Cell::get)
    }
    #[must_use]
    pub fn enter(self) -> RegexEngineGuard {
        RegexEngineGuard(
            ACTIVE_ENGINE.with(|slot| slot.replace(self)),
            std::marker::PhantomData,
        )
    }
    /// Verify that the selected backend can execute.
    ///
    /// # Errors
    /// Returns an error when JIT support is unavailable or its probe cannot compile.
    pub fn ensure_available(self) -> Result<(), String> {
        match self {
            Self::Rust => Ok(()),
            Self::Pcre2Jit => {
                #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
                {
                    pcre2::Pcre2::compile("", 1).map(|_| ())
                }
                #[cfg(not(all(feature = "pcre2-jit", not(target_arch = "wasm32"))))]
                {
                    Err("PCRE2 JIT is unavailable in this binary; enable the pcre2-jit feature on a supported target".into())
                }
            }
        }
    }
}

#[derive(Debug)]
struct Schema {
    names: Vec<Option<String>>,
}

#[derive(Clone, Debug)]
pub struct Regex {
    // Cloning this field deliberately preserves Rust's worker-local cache pool.
    rust: regex::Regex,
    schema: Arc<Schema>,
    scratch: Arc<ThreadLocal<RefCell<regex::CaptureLocations>>>,
    fallbacks: Arc<AtomicU64>,
    #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
    jit: Arc<std::sync::OnceLock<Result<pcre2::Pcre2, String>>>,
}

impl Regex {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.rust.as_str()
    }
    /// Parse a pattern using Rust regex syntax.
    ///
    /// # Errors
    /// Returns an error for invalid Rust regex syntax.
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        regex::Regex::new(pattern).map(Self::from)
    }
    /// Compile a pattern for the selected backend.
    ///
    /// # Errors
    /// Returns an error for invalid syntax or an unsupported backend translation.
    pub fn new_with_engine(pattern: &str, engine: RegexEngine) -> Result<Self, regex::Error> {
        let regex = Self::new(pattern)?;
        regex.prepare(engine)?;
        Ok(regex)
    }
    /// Prepare this pattern for a backend before resolving the program.
    ///
    /// # Errors
    /// Returns an error if the backend is unavailable or cannot compile this pattern.
    pub fn prepare(&self, engine: RegexEngine) -> Result<(), regex::Error> {
        if engine == RegexEngine::Rust {
            return Ok(());
        }
        #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
        {
            self.jit
                .get_or_init(|| pcre2::Pcre2::compile(self.as_str(), self.schema.names.len()))
                .as_ref()
                .map(|_| ())
                .map_err(|e| regex::Error::Syntax(e.clone()))
        }
        #[cfg(not(all(feature = "pcre2-jit", not(target_arch = "wasm32"))))]
        {
            engine.ensure_available().map_err(regex::Error::Syntax)
        }
    }
    #[must_use]
    pub fn as_rust(&self) -> &regex::Regex {
        &self.rust
    }
    #[must_use]
    pub fn fallback_count(&self) -> u64 {
        self.fallbacks.load(Ordering::Relaxed)
    }
    #[must_use]
    pub fn jit_size(&self) -> Option<usize> {
        #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
        {
            self.jit
                .get()
                .and_then(|v| v.as_ref().ok())
                .map(|p| p.jit_bytes)
        }
        #[cfg(not(all(feature = "pcre2-jit", not(target_arch = "wasm32"))))]
        {
            None
        }
    }
    fn fallback(&self, error: &str) {
        #[cfg(feature = "pcre2-jit")]
        metrics::counter!("vrl_regex_fallbacks_total").increment(1);
        if self.fallbacks.fetch_add(1, Ordering::Relaxed) == 0 {
            tracing::warn!(
                message = "PCRE2 JIT could not execute a regex; using the bounded Rust engine for this match.",
                reason = error
            );
        }
    }
    fn start(subject: &str, mut start: usize) -> Option<usize> {
        if start > subject.len() {
            return None;
        }
        while !subject.is_char_boundary(start) {
            start += 1;
        }
        Some(start)
    }
    /// Visit capture groups while reusable matching storage is borrowed.
    ///
    /// # Panics
    /// Propagates callback panics and panics if an internal retry invokes it twice.
    pub fn with_captures<R>(
        &self,
        subject: &str,
        start: usize,
        f: impl FnOnce(&dyn CaptureGroups) -> R,
    ) -> Option<R> {
        let start = Self::start(subject, start)?;
        let mut f = Some(f);
        if RegexEngine::current() == RegexEngine::Pcre2Jit {
            #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
            {
                match self
                    .jit
                    .get_or_init(|| pcre2::Pcre2::compile(self.as_str(), self.schema.names.len()))
                    .as_ref()
                {
                    Ok(jit) => match jit.with_captures(subject, start, |spans| {
                        f.take().expect("callback executes once")(&OffsetView { subject, spans })
                    }) {
                        Ok(result) => return result,
                        Err(error) => self.fallback(&error),
                    },
                    Err(error) => self.fallback(error),
                }
            }
            #[cfg(not(all(feature = "pcre2-jit", not(target_arch = "wasm32"))))]
            self.fallback("PCRE2 JIT is unavailable");
        }
        let scratch = self
            .scratch
            .get_or(|| RefCell::new(self.rust.capture_locations()));
        if let Ok(mut locations) = scratch.try_borrow_mut() {
            self.rust.captures_read_at(&mut locations, subject, start)?;
            Some(f.take().expect("callback executes once")(
                &LocatedCaptures::new(subject, &locations),
            ))
        } else {
            let captures = self.rust.captures_at(subject, start)?;
            Some(f.take().expect("callback executes once")(&captures))
        }
    }
    #[must_use]
    pub fn captures<'h>(&self, subject: &'h str) -> Option<Captures<'h>> {
        self.captures_at(subject, 0)
    }
    #[must_use]
    pub fn captures_at<'h>(&self, subject: &'h str, start: usize) -> Option<Captures<'h>> {
        self.with_captures(subject, start, |groups| Captures {
            subject,
            spans: (0..groups.group_len())
                .map(|i| groups.group(i).map(|m| (m.start, m.end)))
                .collect(),
            schema: self.schema.clone(),
        })
    }
    #[must_use]
    pub fn is_match(&self, subject: &str) -> bool {
        if RegexEngine::current() == RegexEngine::Rust {
            self.rust.is_match(subject)
        } else {
            self.with_captures(subject, 0, |_| ()).is_some()
        }
    }
    #[must_use]
    pub fn find<'h>(&self, subject: &'h str) -> Option<Match<'h>> {
        self.find_at(subject, 0)
    }
    #[must_use]
    /// Find a match at or after a byte offset.
    ///
    /// # Panics
    /// Panics if a backend violates the invariant that a match contains group zero.
    pub fn find_at<'h>(&self, subject: &'h str, start: usize) -> Option<Match<'h>> {
        let start = Self::start(subject, start)?;
        if RegexEngine::current() == RegexEngine::Rust {
            self.rust.find_at(subject, start).map(|m| Match {
                subject,
                start: m.start(),
                end: m.end(),
            })
        } else {
            self.with_captures(subject, start, |groups| {
                let m = groups.group(0).expect("a successful search has group zero");
                Match {
                    subject,
                    start: m.start,
                    end: m.end,
                }
            })
        }
    }
    #[must_use]
    pub fn find_iter<'r, 'h>(&'r self, subject: &'h str) -> Matches<'r, 'h> {
        Matches {
            regex: self,
            subject,
            position: Some(0),
            last_end: None,
        }
    }
    #[must_use]
    pub fn captures_iter<'r, 'h>(&'r self, subject: &'h str) -> CaptureMatches<'r, 'h> {
        CaptureMatches {
            regex: self,
            subject,
            position: Some(0),
            last_end: None,
        }
    }
    #[must_use]
    pub fn splitn<'r, 'h>(&'r self, subject: &'h str, limit: usize) -> SplitN<'r, 'h> {
        SplitN {
            matches: self.find_iter(subject),
            last: 0,
            remaining: limit,
            done: limit == 0,
        }
    }
    pub fn replace_all<'h, R: Replacer>(&self, subject: &'h str, replacer: R) -> Cow<'h, str> {
        self.replacen(subject, 0, replacer)
    }
    /// Replace at most `limit` matches; zero means all matches.
    ///
    /// # Panics
    /// Propagates replacer panics and panics if a match lacks group zero.
    pub fn replacen<'h, R: Replacer>(
        &self,
        subject: &'h str,
        limit: usize,
        mut replacer: R,
    ) -> Cow<'h, str> {
        let max = if limit == 0 { usize::MAX } else { limit };
        let mut result = String::new();
        let mut last = 0;
        let mut count = 0;
        if let Some(text) = replacer.no_expansion() {
            for m in self.find_iter(subject).take(max) {
                if count == 0 {
                    result.reserve(subject.len());
                }
                result.push_str(&subject[last..m.start()]);
                result.push_str(&text);
                last = m.end();
                count += 1;
            }
        } else {
            for captures in self.captures_iter(subject).take(max) {
                let m = captures.get(0).expect("a match has group zero");
                if count == 0 {
                    result.reserve(subject.len());
                }
                result.push_str(&subject[last..m.start()]);
                replacer.replace_append(&captures, &mut result);
                last = m.end();
                count += 1;
            }
        }
        if count == 0 {
            Cow::Borrowed(subject)
        } else {
            result.push_str(&subject[last..]);
            Cow::Owned(result)
        }
    }
}

impl From<regex::Regex> for Regex {
    fn from(rust: regex::Regex) -> Self {
        let schema = Arc::new(Schema {
            names: rust.capture_names().map(|s| s.map(str::to_owned)).collect(),
        });
        Self {
            rust,
            schema,
            scratch: Arc::new(ThreadLocal::new()),
            fallbacks: Arc::new(AtomicU64::new(0)),
            #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
            jit: Arc::new(std::sync::OnceLock::new()),
        }
    }
}
impl Deref for Regex {
    type Target = regex::Regex;
    fn deref(&self) -> &Self::Target {
        &self.rust
    }
}
impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.rust, f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Match<'h> {
    subject: &'h str,
    start: usize,
    end: usize,
}
impl<'h> Match<'h> {
    #[must_use]
    pub fn start(&self) -> usize {
        self.start
    }
    #[must_use]
    pub fn end(&self) -> usize {
        self.end
    }
    #[must_use]
    pub fn as_str(&self) -> &'h str {
        &self.subject[self.start..self.end]
    }
    #[must_use]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

type Span = Option<(usize, usize)>;
#[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
struct OffsetView<'a> {
    subject: &'a str,
    spans: &'a [Span],
}
#[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
impl CaptureGroups for OffsetView<'_> {
    fn group_len(&self) -> usize {
        self.spans.len()
    }
    fn group(&self, index: usize) -> Option<CaptureView<'_>> {
        self.spans
            .get(index)
            .copied()
            .flatten()
            .map(|(start, end)| CaptureView {
                start,
                end,
                text: &self.subject[start..end],
            })
    }
}

#[derive(Debug)]
pub struct Captures<'h> {
    subject: &'h str,
    spans: Vec<Span>,
    schema: Arc<Schema>,
}
impl<'h> Captures<'h> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Match<'h>> {
        self.spans
            .get(index)
            .copied()
            .flatten()
            .map(|(start, end)| Match {
                subject: self.subject,
                start,
                end,
            })
    }
    #[must_use]
    pub fn name(&self, name: &str) -> Option<Match<'h>> {
        self.schema
            .names
            .iter()
            .position(|n| n.as_deref() == Some(name))
            .and_then(|i| self.get(i))
    }
    pub fn iter(&self) -> impl Iterator<Item = Option<Match<'h>>> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }
    pub fn expand(&self, replacement: &str, result: &mut String) {
        regex_automata::util::interpolate::string(
            replacement,
            |i, dst| {
                if let Some(m) = self.get(i) {
                    dst.push_str(m.as_str());
                }
            },
            |name| {
                self.schema
                    .names
                    .iter()
                    .position(|n| n.as_deref() == Some(name))
            },
            result,
        );
    }
}
impl Index<usize> for Captures<'_> {
    type Output = str;
    fn index(&self, index: usize) -> &str {
        self.get(index)
            .expect("capture does not participate")
            .as_str()
    }
}

impl Index<&str> for Captures<'_> {
    type Output = str;
    fn index(&self, name: &str) -> &str {
        self.name(name)
            .expect("named capture did not participate")
            .as_str()
    }
}
impl CaptureGroups for Captures<'_> {
    fn group_len(&self) -> usize {
        self.len()
    }
    fn group(&self, index: usize) -> Option<CaptureView<'_>> {
        self.get(index).map(|m| CaptureView {
            start: m.start(),
            end: m.end(),
            text: m.as_str(),
        })
    }
}

fn advance(subject: &str, at: usize) -> Option<usize> {
    subject[at..].chars().next().map(|c| at + c.len_utf8())
}
fn next_position(
    subject: &str,
    start: usize,
    end: usize,
    previous: Option<usize>,
) -> (Option<usize>, bool) {
    if start == end {
        (advance(subject, end), previous == Some(end))
    } else {
        (Some(end), false)
    }
}
pub struct Matches<'r, 'h> {
    regex: &'r Regex,
    subject: &'h str,
    position: Option<usize>,
    last_end: Option<usize>,
}
impl<'h> Iterator for Matches<'_, 'h> {
    type Item = Match<'h>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let m = self.regex.find_at(self.subject, self.position?)?;
            let (next, skip) = next_position(self.subject, m.start(), m.end(), self.last_end);
            self.position = next;
            if skip {
                continue;
            }
            self.last_end = Some(m.end());
            return Some(m);
        }
    }
}
pub struct CaptureMatches<'r, 'h> {
    regex: &'r Regex,
    subject: &'h str,
    position: Option<usize>,
    last_end: Option<usize>,
}
impl<'h> Iterator for CaptureMatches<'_, 'h> {
    type Item = Captures<'h>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let capture = self.regex.captures_at(self.subject, self.position?)?;
            let m = capture.get(0).expect("a match has group zero");
            let (next, skip) = next_position(self.subject, m.start(), m.end(), self.last_end);
            self.position = next;
            if skip {
                continue;
            }
            self.last_end = Some(m.end());
            return Some(capture);
        }
    }
}
pub struct SplitN<'r, 'h> {
    matches: Matches<'r, 'h>,
    last: usize,
    remaining: usize,
    done: bool,
}
impl<'h> Iterator for SplitN<'_, 'h> {
    type Item = &'h str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        self.remaining -= 1;
        if self.remaining != 0
            && let Some(m) = self.matches.next()
        {
            let part = &self.matches.subject[self.last..m.start()];
            self.last = m.end();
            return Some(part);
        }
        self.done = true;
        Some(&self.matches.subject[self.last..])
    }
}

pub trait Replacer {
    fn replace_append(&mut self, captures: &Captures<'_>, dst: &mut String);
    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        None
    }
}
impl Replacer for &str {
    fn replace_append(&mut self, captures: &Captures<'_>, dst: &mut String) {
        captures.expand(self, dst);
    }
    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        (!self.contains('$')).then_some(Cow::Borrowed(self))
    }
}

#[derive(Clone, Debug)]
pub struct RegexSet {
    rust: regex::bytes::RegexSet,
    regexes: Vec<Regex>,
}
impl RegexSet {
    /// Compile a set using the default Rust backend.
    ///
    /// # Errors
    /// Returns an error if a pattern has invalid syntax.
    pub fn new(patterns: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, regex::Error> {
        Self::new_with_engine(patterns, RegexEngine::Rust)
    }
    /// Compile a set for the selected backend.
    ///
    /// # Errors
    /// Returns an error for invalid syntax or an unsupported backend translation.
    pub fn new_with_engine(
        patterns: impl IntoIterator<Item = impl AsRef<str>>,
        engine: RegexEngine,
    ) -> Result<Self, regex::Error> {
        let regexes: Vec<_> = patterns
            .into_iter()
            .map(|p| Regex::new_with_engine(p.as_ref(), engine))
            .collect::<Result<_, _>>()?;
        let rust = regex::bytes::RegexSet::new(regexes.iter().map(Regex::as_str))?;
        Ok(Self { rust, regexes })
    }
    #[must_use]
    pub fn is_match(&self, subject: &[u8]) -> bool {
        if RegexEngine::current() == RegexEngine::Rust {
            return self.rust.is_match(subject);
        }
        if let Ok(subject) = std::str::from_utf8(subject) {
            self.regexes.iter().any(|r| r.is_match(subject))
        } else {
            // Preserve match_any's arbitrary-byte behavior without passing
            // invalid UTF-8 to the JIT API.
            if let Some(regex) = self.regexes.first() {
                regex.fallback("byte regex-set input contains invalid UTF-8");
            }
            self.rust.is_match(subject)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_restore_after_nested_execution_and_unwinding() {
        assert_eq!(RegexEngine::current(), RegexEngine::Rust);
        let outer = RegexEngine::Pcre2Jit.enter();
        assert_eq!(RegexEngine::current(), RegexEngine::Pcre2Jit);
        {
            let _inner = RegexEngine::Rust.enter();
            assert_eq!(RegexEngine::current(), RegexEngine::Rust);
        }
        assert_eq!(RegexEngine::current(), RegexEngine::Pcre2Jit);
        let _panic = std::panic::catch_unwind(|| {
            let _inner = RegexEngine::Rust.enter();
            panic!("exercise restoration");
        });
        assert_eq!(RegexEngine::current(), RegexEngine::Pcre2Jit);
        drop(outer);
        assert_eq!(RegexEngine::current(), RegexEngine::Rust);
    }

    #[test]
    fn rust_and_selected_backends_preserve_captures_iteration_split_and_replacement() {
        let mut engines = vec![RegexEngine::Rust];
        if RegexEngine::Pcre2Jit.ensure_available().is_ok() {
            engines.push(RegexEngine::Pcre2Jit);
        }
        for pattern in [
            "",
            "a*",
            "a|$",
            r"(?P<a>a)(?P<b>b)?",
            r"(a)?(b)",
            r"(?m)^",
            r"(?m)$",
            r"(?mR)^|$",
            r"[a-z&&[^aeiou]]+",
            r"\b\w+\b",
            r"\B",
            r"(?i)ä+",
            r"[a&&b]",
        ] {
            let original = regex::Regex::new(pattern).unwrap();
            for engine in &engines {
                let regex = Regex::new_with_engine(pattern, *engine).unwrap();
                let _scope = engine.enter();
                for subject in [
                    "", "a", "ab", "abc", "abc\n", "é🙂", "a\0b", "one two", "\r\n", "e\u{301}",
                    "\u{200d}", "Ää",
                ] {
                    assert_eq!(
                        regex.is_match(subject),
                        original.is_match(subject),
                        "{pattern:?}/{subject:?}/{engine:?}"
                    );
                    let expected: Vec<_> = original.find_iter(subject).map(|m| m.range()).collect();
                    assert_eq!(
                        regex
                            .find_iter(subject)
                            .map(|m| m.range())
                            .collect::<Vec<_>>(),
                        expected,
                        "{pattern:?}/{subject:?}/{engine:?}"
                    );
                    let expected: Vec<_> = original
                        .captures_iter(subject)
                        .map(|c| c.iter().map(|g| g.map(|m| m.range())).collect::<Vec<_>>())
                        .collect();
                    assert_eq!(
                        regex
                            .captures_iter(subject)
                            .map(|c| c.iter().map(|g| g.map(|m| m.range())).collect::<Vec<_>>())
                            .collect::<Vec<_>>(),
                        expected,
                        "{pattern:?}/{subject:?}/{engine:?}"
                    );
                    for limit in [0, 1, 2, 4, usize::MAX] {
                        assert_eq!(
                            regex.splitn(subject, limit).collect::<Vec<_>>(),
                            original.splitn(subject, limit).collect::<Vec<_>>(),
                            "split {pattern:?}/{subject:?}/{limit}/{engine:?}"
                        );
                    }
                    for with in ["x", "$0:$a:$2:$$:${b}", "$missing", "${a}é"] {
                        assert_eq!(
                            regex.replace_all(subject, with),
                            original.replace_all(subject, with),
                            "replace {pattern:?}/{subject:?}/{with:?}/{engine:?}"
                        );
                    }
                }
                assert_eq!(regex.fallback_count(), 0);
                if *engine == RegexEngine::Pcre2Jit {
                    assert!(regex.jit_size().unwrap() > 0);
                }
            }
        }
    }

    #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
    #[test]
    fn jit_worker_scratch_supports_reentry_and_concurrent_programs() {
        let regex =
            Arc::new(Regex::new_with_engine(r"(?P<a>a)(?P<b>b)?", RegexEngine::Pcre2Jit).unwrap());
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let regex = regex.clone();
                std::thread::spawn(move || {
                    let selected = if i % 2 == 0 {
                        RegexEngine::Rust
                    } else {
                        RegexEngine::Pcre2Jit
                    };
                    let _scope = selected.enter();
                    for _ in 0..100 {
                        regex
                            .with_captures("ab", 0, |_| {
                                assert_eq!(regex.captures("a").unwrap().get(2), None);
                            })
                            .unwrap();
                        assert_eq!(RegexEngine::current(), selected);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(regex.fallback_count(), 0);
        assert_eq!(RegexEngine::current(), RegexEngine::Rust);
    }

    #[cfg(all(feature = "pcre2-jit", not(target_arch = "wasm32")))]
    #[test]
    fn jit_work_limit_and_invalid_byte_sets_fall_back_without_losing_results() {
        let _scope = RegexEngine::Pcre2Jit.enter();
        let regex = Regex::new_with_engine(r"^(a|aa)+$", RegexEngine::Pcre2Jit).unwrap();
        let subject = format!("{}!", "a".repeat(100));
        assert!(!regex.is_match(&subject));
        assert_eq!(
            regex.fallback_count(),
            1,
            "native work limit must be observable"
        );
        let mut calls = 0;
        assert_eq!(
            regex.with_captures("aaaa", 0, |_| {
                calls += 1;
                42
            }),
            Some(42)
        );
        assert_eq!(calls, 1);
        let set = RegexSet::new_with_engine(["a", "b"], RegexEngine::Pcre2Jit).unwrap();
        assert!(set.is_match(b"\xffa"));
        assert_eq!(set.regexes[0].fallback_count(), 1);
    }

    #[cfg(all(
        feature = "pcre2-jit",
        not(target_arch = "wasm32"),
        feature = "stdlib-base"
    ))]
    #[test]
    fn every_public_regex_operator_uses_the_configured_program_backend() {
        use crate::{
            compiler::{
                CompileConfig, TimeZone, compile_with_external, runtime::Runtime,
                state::ExternalEnv,
            },
            value::{Value, regex_capture::RegexCaptureMapMode},
        };
        let expressions = [
            r#"match("abc", r'^a')"#,
            r#"match_array(["aa", "ab"], r'^a', all: true)"#,
            r#"match_any("ab", [r'^a', r'^b'])"#,
            r#"parse_regex!("one two", r'(?P<a>\w+) (?P<b>\w+)')"#,
            r#"parse_regex_all!("é a", r'(?P<word>\w+)')"#,
            r#"replace("ab ab", r'(?P<letter>a)b', "${letter}")"#,
            r#"replace_with("a b", r'(?P<word>\w+)') -> |m| { upcase!(m.string) }"#,
            r#"split("a,b,c", r',', limit: 2)"#,
            r#"find("éabc", r'a', from: 1)"#,
            r#"redact("my123id", filters: [r'\d+'])"#,
            r#"redact("123-12-1234", filters: ["us_social_security_number"])"#,
            r#"sieve("a1b2", r'[a-z]')"#,
            r#"match("abc", to_regex!("^a"))"#,
            r#"to_regex!("(?P<probe>.)")"#,
            r#"parse_duration!("1h 30m", "s")"#,
            r#"parse_glog!("I20210131 14:48:54.411655 15520 main.c++:9] Hello world!")"#,
            r#"parse_klog!("I0505 17:59:40.692994   28133 klog.go:70] hello from klog")"#,
            r#"parse_nginx_log!("2020/12/25 12:00:00 [error] 1#1: hello", "error")"#,
        ];
        for source in expressions {
            let mut expected = None;
            for mode in [RegexCaptureMapMode::Owned, RegexCaptureMapMode::Shared] {
                for engine in [RegexEngine::Rust, RegexEngine::Pcre2Jit] {
                    let mut config = CompileConfig::default();
                    config.set_custom(engine);
                    config.set_custom(mode);
                    let program = compile_with_external(
                        source,
                        &crate::stdlib::all(),
                        &ExternalEnv::default(),
                        config,
                    )
                    .unwrap()
                    .program;
                    let before = JIT_EXECUTIONS.with(Cell::get);
                    let mut target = Value::Object(Default::default());
                    let value = Runtime::default()
                        .resolve(&mut target, &program, &TimeZone::default())
                        .unwrap();
                    let calls = JIT_EXECUTIONS.with(Cell::get) - before;
                    assert_eq!(RegexEngine::current(), RegexEngine::Rust);
                    if source.starts_with("to_regex!") {
                        assert_eq!(
                            value.as_engine_regex().unwrap().jit_size().is_some(),
                            engine == RegexEngine::Pcre2Jit
                        );
                    } else {
                        assert_eq!(
                            calls > 0,
                            engine == RegexEngine::Pcre2Jit,
                            "actual JIT invocation for {source}"
                        );
                    }
                    match &expected {
                        Some(expected) => assert_eq!(&value, expected, "{source}"),
                        None => expected = Some(value),
                    }
                }
            }
        }
    }
}
