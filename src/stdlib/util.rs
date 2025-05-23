use crate::compiler::{Context, Expression, Resolved, TypeState};
use crate::value::{KeyString, ObjectMap, Value};

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

/// Takes a set of captures that have resulted from matching a regular expression
/// against some text and fills a `BTreeMap` with the result.
///
/// All captures are inserted with a key as the numeric index of that capture
/// "0" is the overall match.
/// Any named captures are also added to the Map with the key as the name.
///
pub(crate) fn capture_regex_to_map(
    regex: &regex::Regex,
    capture: &regex::Captures,
    numeric_groups: bool,
) -> ObjectMap {
    let names = regex.capture_names().flatten().map(|name| {
        (
            name.to_owned().into(),
            capture.name(name).map(|s| s.as_str()).into(),
        )
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
