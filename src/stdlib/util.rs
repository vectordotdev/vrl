cfg_if::cfg_if! {
    if #[cfg(feature = "enable_system_functions")] {
        use relative_path::PathExt;
        use std::{env, path::PathBuf};
    }
}

use crate::value::{KeyString, ObjectMap, Value};

#[cfg(feature = "enable_crypto_functions")]
use bytes::Bytes;
#[cfg(feature = "enable_crypto_functions")]
use bytestring::ByteString;

pub(crate) const DYNAMIC_REGEX_NOTICE: &str = indoc::indoc! {"
    When `pattern` is a dynamic expression (e.g. a variable or the result of `to_regex`),
    the regex is compiled on every function call. For high-throughput pipelines, prefer
    a regex literal so the pattern is compiled once at program compile time.
"};

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

/// Hex-encode a digest into a `ByteString`.
///
/// `N` is the hex length in bytes (twice the digest length). Hex is ASCII, so the
/// result is UTF-8 by construction.
#[cfg(feature = "enable_crypto_functions")]
#[inline]
pub(crate) fn hex_encode<const N: usize>(digest: impl AsRef<[u8]>) -> ByteString {
    let mut buf = [0u8; N];
    hex::encode_to_slice(digest, &mut buf).expect("hex buffer sized for digest");
    // SAFETY: hexadecimal encoding is ASCII.
    unsafe { ByteString::from_bytes_unchecked(Bytes::copy_from_slice(&buf)) }
}

#[derive(Debug, Clone)]
pub(crate) struct RegexWithCaptureInfo {
    pub regex: regex::Regex,
    pub capture_info: Vec<(KeyString, usize)>,
}

impl RegexWithCaptureInfo {
    pub fn new(regex: regex::Regex) -> Self {
        let capture_info = build_capture_info(&regex);
        Self {
            regex,
            capture_info,
        }
    }
}

pub(crate) fn build_capture_info(regex: &regex::Regex) -> Vec<(KeyString, usize)> {
    regex
        .capture_names()
        .enumerate()
        .filter_map(|(i, name)| name.map(|n| (KeyString::from(n), i)))
        .collect()
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
pub(crate) fn capture_regex_to_map(
    capture: &regex::Captures,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
) -> ObjectMap {
    let names = capture_info.iter().map(|(name, idx)| {
        let value: Value = capture.get(*idx).map(|m| m.as_str()).into();
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
        Value::String(s) => s.is_empty() || &**s == "-" || s.chars().all(char::is_whitespace),
        Value::Bytes(b) => {
            b.is_empty()
                || b.as_ref() == b"-"
                || String::from_utf8_lossy(b).chars().all(char::is_whitespace)
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
