cfg_if::cfg_if! {
    if #[cfg(feature = "enable_system_functions")] {
        use relative_path::PathExt;
        use std::{env, path::PathBuf};
    }
}

use crate::compiler::{Context, Expression, Resolved, TypeState};
use crate::value::{KeyString, ObjectMap, Value};

#[cfg(feature = "enable_network_functions")]
pub(crate) const NETWORK_CALL_NOTICE: &str = indoc::indoc! {"
    This function performs synchronous blocking operations and is not recommended for
    frequent or performance-critical workflows due to potential network-related delays.
"};

/// Rounds the given number to the given precision.
/// Takes a function parameter so the exact rounding function (ceil, floor or round)
/// can be specified.
#[inline]
#[allow(clippy::cast_precision_loss)] //TODO evaluate removal options
pub(crate) fn round_to_precision<F>(num: f64, precision: i64, fun: F) -> f64
where
    F: Fn(f64) -> f64,
{
    let multiplier = 10_f64.powf(precision as f64);
    fun(num * multiplier) / multiplier
}

/// Returns `true` when zero-copy slicing is worthwhile for this capture set.
///
/// Zero-copy is used when:
/// - The input is `< 64` bytes: the source buffer is small enough that
///   retaining it is cheaper than the per-value [`bytes::Bytes`] overhead.
/// - The total length of all named captures is at least half the input
///   length: most of the retained buffer is actually used.
///
/// `capture_info` must contain pre-computed group indices so that
/// [`regex::Captures::get`] (direct array access) is used instead of
/// name-based hash lookups.
pub(crate) fn should_zero_copy(
    input_len: usize,
    capture: &regex::Captures,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
) -> bool {
    if input_len < 64 {
        return true;
    }

    let emitted_capture_count = capture_info.len() + if numeric_groups { capture.len() } else { 0 };
    if emitted_capture_count == 0 {
        return false;
    }

    if !numeric_groups && capture_info.len() >= 4 {
        if let (Some((_, first_idx)), Some((_, last_idx))) =
            (capture_info.first(), capture_info.last())
            && let (Some(first), Some(last)) = (capture.get(*first_idx), capture.get(*last_idx))
            && last.end().saturating_sub(first.start()) * 2 >= input_len
        {
            return true;
        }
    }

    if let Some(full_match) = capture.get(0) {
        let full_match_len = full_match.end() - full_match.start();
        if full_match_len
            .saturating_mul(emitted_capture_count)
            .saturating_mul(2)
            < input_len
        {
            return false;
        }
    }

    let mut total = 0usize;

    if numeric_groups {
        for m in capture.iter().flatten() {
            total += m.end() - m.start();
            if total * 2 >= input_len {
                return true;
            }
        }
    }

    for (_, i) in capture_info.iter().rev() {
        if let Some(m) = capture.get(*i) {
            total += m.end() - m.start();
            if total * 2 >= input_len {
                return true;
            }
        }
    }

    false
}

/// Fills an [`ObjectMap`] from a regex [`Captures`](regex::Captures).
///
/// Named captures are inserted under their group name; numeric groups (when
/// `numeric_groups` is `true`) are inserted under their zero-based index, with
/// `"0"` holding the full match.
///
/// `capture_info` must be the pre-computed `(name, group_index)` slice
/// (computed once at VRL compile time).  Group indices allow direct O(1)
/// array access via [`regex::Captures::get`] instead of name-based hash
/// lookups.
///
/// When `utf8_bytes` is `Some`, each matched substring is returned as a
/// zero-copy [`bytes::Bytes`] slice of that buffer; when `None` each is copied.
pub(crate) fn capture_regex_to_map(
    capture: &regex::Captures,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
    utf8_bytes: Option<&bytes::Bytes>,
) -> ObjectMap {
    match utf8_bytes {
        Some(utf8_bytes) => {
            capture_regex_to_map_zero_copy(capture, capture_info, numeric_groups, utf8_bytes)
        }
        None => capture_regex_to_map_copy(capture, capture_info, numeric_groups),
    }
}

pub(crate) fn capture_regex_to_map_from_template(
    capture: &regex::Captures,
    capture_info_by_key: &[(KeyString, usize)],
    template: &ObjectMap,
    utf8_bytes: Option<&bytes::Bytes>,
) -> ObjectMap {
    let mut map = template.clone();

    for ((_, idx), value) in capture_info_by_key.iter().zip(map.values_mut()) {
        *value = match (capture.get(*idx), utf8_bytes) {
            (Some(m), Some(b)) => b.slice(m.start()..m.end()).into(),
            (Some(m), None) => m.as_str().into(),
            (None, _) => Value::Null,
        };
    }

    map
}

fn capture_regex_to_map_copy(
    capture: &regex::Captures,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
) -> ObjectMap {
    let names = capture_info.iter().map(|(name, idx)| {
        let value: Value = match capture.get(*idx) {
            Some(m) => m.as_str().into(),
            None => Value::Null,
        };
        (name.clone(), value)
    });

    if numeric_groups {
        let indexed = capture
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, c)| (KeyString::from(idx.to_string()), c.as_str().into()));
        indexed.chain(names).collect()
    } else {
        names.collect()
    }
}

fn capture_regex_to_map_zero_copy(
    capture: &regex::Captures,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
    utf8_bytes: &bytes::Bytes,
) -> ObjectMap {
    let names = capture_info.iter().map(|(name, idx)| {
        let value: Value = match capture.get(*idx) {
            Some(m) => utf8_bytes.slice(m.start()..m.end()).into(),
            None => Value::Null,
        };
        (name.clone(), value)
    });

    if numeric_groups {
        let indexed = capture.iter().flatten().enumerate().map(|(idx, c)| {
            (
                KeyString::from(idx.to_string()),
                utf8_bytes.slice(c.start()..c.end()).into(),
            )
        });
        indexed.chain(names).collect()
    } else {
        names.collect()
    }
}

pub(crate) fn regex_kind(
    regex: &regex::Regex,
) -> std::collections::BTreeMap<crate::value::kind::Field, crate::value::kind::Kind> {
    use crate::value::kind::Kind;

    let mut inner_type = std::collections::BTreeMap::new();

    // Add typedefs for each capture by numerical index.
    for num in 0..regex.captures_len() {
        inner_type.insert(num.to_string().into(), Kind::bytes() | Kind::null());
    }

    // Add a typedef for each capture name.
    for name in regex.capture_names().flatten() {
        inner_type.insert(name.to_owned().into(), Kind::bytes());
    }

    inner_type
}

pub(crate) fn is_nullish(value: &Value) -> bool {
    match value {
        Value::Bytes(v) => {
            if v.is_empty() || v.as_ref() == b"-" {
                return true;
            }

            let s = value.as_str().expect("value should be bytes");
            s.chars().all(char::is_whitespace)
        }
        Value::Null => true,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Base64Charset {
    #[default]
    Standard,
    UrlSafe,
}

impl Base64Charset {
    pub(super) fn from_slice(bytes: &[u8]) -> Result<Self, &'static str> {
        match bytes {
            b"standard" => Ok(Self::Standard),
            b"url_safe" => Ok(Self::UrlSafe),
            _ => Err("unknown charset"),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum ConstOrExpr {
    Const(Value),
    Expr(Box<dyn Expression>),
}

impl ConstOrExpr {
    pub(super) fn new(expr: Box<dyn Expression>, state: &TypeState) -> Self {
        match expr.resolve_constant(state) {
            Some(cnst) => Self::Const(cnst),
            None => Self::Expr(expr),
        }
    }

    pub(super) fn optional(expr: Option<Box<dyn Expression>>, state: &TypeState) -> Option<Self> {
        expr.map(|expr| Self::new(expr, state))
    }

    pub(super) fn resolve(&self, ctx: &mut Context) -> Resolved {
        match self {
            Self::Const(value) => Ok(value.clone()),
            Self::Expr(expr) => expr.resolve(ctx),
        }
    }
}

/// Only to be used in examples since this can return an incorrect path.
/// Useful for displaying a nicer path in the docs, since the path is going to be incorrect when
/// docs are generated from outside this repo.
///
/// Get actual path as a string if exists or basename if not.
///
/// `input` path is relative to `tests/data/`.
#[cfg(feature = "enable_system_functions")]
pub(crate) fn example_path_or_basename(input: &'static str) -> String {
    let manifest_dir =
        env::var_os("CARGO_MANIFEST_DIR").map(|dir| PathBuf::from(dir).join("../.."));
    let path = manifest_dir
        .clone()
        .map(|dir| dir.join("tests/data").join(input));

    let not_found_default = || {
        // Mock repo root
        PathBuf::from("tests/data")
            .join(input)
            .display()
            .to_string()
    };

    if let Some(manifest_dir) = manifest_dir
        && let Some(path) = path
        && path.exists()
    {
        path.relative_to(manifest_dir)
            .map_or_else(|_| not_found_default(), String::from)
    } else {
        not_found_default()
    }
}
