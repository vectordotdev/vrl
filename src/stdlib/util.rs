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

/// Fills an [`ObjectMap`] from a regex [`Captures`](regex::Captures).
///
/// Named captures are inserted under their group name; numeric groups (when
/// `numeric_groups` is `true`) are inserted under their zero-based index, with
/// `"0"` holding the full match.
///
/// `capture_names` must be the pre-computed slice of named-group
/// [`KeyString`]s for the regex (computed once at VRL compile time via
/// `regex.capture_names().flatten().map(KeyString::from)`).
///
/// When `original_bytes` is `Some`, each matched substring is produced as a
/// zero-copy [`bytes::Bytes`] slice of the original input buffer rather than a
/// heap copy.  Only pass `Some` when the input is valid UTF-8, so that the
/// byte offsets returned by the regex are valid positions in the buffer.
pub(crate) fn capture_regex_to_map(
    capture: &regex::Captures,
    capture_names: &[KeyString],
    numeric_groups: bool,
    original_bytes: Option<&bytes::Bytes>,
) -> ObjectMap {
    let names = capture_names.iter().map(|name| {
        let value: Value = match capture.name(name.as_str()) {
            Some(m) => match original_bytes {
                Some(b) => b.slice(m.start()..m.end()).into(),
                None => m.as_str().into(),
            },
            None => Value::Null,
        };
        (name.clone(), value)
    });

    if numeric_groups {
        let indexed = capture.iter().flatten().enumerate().map(|(idx, c)| {
            let value: Value = match original_bytes {
                Some(b) => b.slice(c.start()..c.end()).into(),
                None => c.as_str().into(),
            };
            (KeyString::from(idx.to_string()), value)
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
