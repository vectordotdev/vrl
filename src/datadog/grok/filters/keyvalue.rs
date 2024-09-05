use std::collections::BTreeMap;
use std::fmt::Formatter;

use crate::value::Value;
use bytes::Bytes;
use fancy_regex::{Captures, Regex};
use nom::combinator::eof;
use nom::{
    self,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::char,
    combinator::{map, opt, rest, value},
    number::complete::double,
    sequence::{delimited, terminated},
    IResult, Parser,
};
use onig::EncodedChars;
use ordered_float::NotNan;

use super::super::{
    ast::{Function, FunctionArgument},
    grok_filter::GrokFilter,
    parse_grok::Error as GrokRuntimeError,
    parse_grok_rules::Error as GrokStaticError,
};

const DEFAULT_VALUE_RE: &str = r"\w.\-_@";
const DEFAULT_DELIMITERS: (&str, &str) = (r"\s,;(\[{", r"\s,;)\]}");

const DEFAULT_KEYVALUE_DELIMITER: &str = "=";
const DEFAULT_QUOTES: &[(char, char)] = &[('"', '"'), ('\'', '\''), ('<', '>')];

pub fn filter_from_function(f: &Function) -> Result<GrokFilter, GrokStaticError> {
    {
        let args_len = f.args.as_ref().map_or(0, |args| args.len());

        let key_value_delimiter = if args_len > 0 {
            match f.args.as_ref().unwrap()[0] {
                FunctionArgument::Arg(Value::Bytes(ref bytes)) => &String::from_utf8_lossy(bytes),
                _ => return Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            }
        } else {
            DEFAULT_KEYVALUE_DELIMITER
        };
        let value_re = if args_len > 1 {
            match f.args.as_ref().unwrap()[1] {
                FunctionArgument::Arg(Value::Bytes(ref bytes)) => {
                    [DEFAULT_VALUE_RE, &String::from_utf8_lossy(bytes)].concat()
                }
                _ => return Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            }
        } else {
            // default allowed unescaped symbols
            DEFAULT_VALUE_RE.to_string()
        };

        let quotes = if args_len > 2 {
            match f.args.as_ref().unwrap()[2] {
                FunctionArgument::Arg(Value::Bytes(ref bytes)) => {
                    let pair = String::from_utf8_lossy(bytes);
                    match pair {
                        pair if pair.len() == 2 => {
                            let mut chars = pair.chars();
                            Ok(vec![(
                                chars.next().expect("open quote"),
                                chars.next().expect("closing quote"),
                            )])
                        }
                        pair if pair.is_empty() => Ok(Vec::from(DEFAULT_QUOTES)),
                        _ => Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
                    }
                }
                _ => Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            }
        } else {
            Ok(Vec::from(DEFAULT_QUOTES))
        }?;

        let field_delimiters = if args_len > 3 {
            match f.args.as_ref().unwrap()[3] {
                FunctionArgument::Arg(Value::Bytes(ref bytes)) => {
                    let delimiter = String::from_utf8_lossy(bytes).to_string();
                    match (&delimiter[..1], &delimiter[1..]) {
                        (left, right) if !right.is_empty() => (left.to_string(), right.to_string()),
                        (single, "") => (single.to_string(), single.to_string()),
                        _ => return Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
                    }
                }
                _ => return Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            }
        } else {
            (
                DEFAULT_DELIMITERS.0.to_string(),
                DEFAULT_DELIMITERS.1.to_string(),
            )
        };

        Ok(GrokFilter::KeyValue(KeyValueFilter {
            re_pattern: regex_from_config(
                key_value_delimiter,
                value_re,
                quotes.clone(),
                field_delimiters,
            )?,
            quotes,
            key_value_delimiter: key_value_delimiter.to_string(),
        }))
    }
}

fn regex_from_config(
    key_value_delimiter: &str,
    value_re: String,
    quotes: Vec<(char, char)>,
    _field_delimiters: (String, String),
) -> Result<Regex, GrokStaticError> {
    // start group
    let mut quoting = String::from("(");
    // add quotes with OR
    for (left, right) in quotes {
        quoting.push_str(&regex::escape(&left.to_string()));
        quoting.push_str("[^");
        quoting.push_str(&regex::escape(&left.to_string()));
        quoting.push_str("]+");
        quoting.push_str(&regex::escape(&right.to_string()));
        quoting.push('|');
    }

    quoting.push('[');

    let mut keyvalue = String::from("(?<=[");
    keyvalue.push_str(&_field_delimiters.0);
    keyvalue.push_str("]|^)");

    // key
    keyvalue.push_str(quoting.as_str());
    keyvalue.push_str(&value_re);
    keyvalue.push_str("]+)");

    // delimiter
    keyvalue.push_str(key_value_delimiter);

    // value
    keyvalue.push_str(quoting.as_str());
    keyvalue.push_str(&value_re);
    keyvalue.push_str("]+)");

    keyvalue.push_str("(?:[");
    keyvalue.push_str(&_field_delimiters.1);
    keyvalue.push_str("]|$)");

    Regex::new(keyvalue.as_str())
        .map_err(|_e| GrokStaticError::InvalidFunctionArguments("keyvalue".to_string()))
}

#[derive(Debug, Clone)]
pub struct KeyValueFilter {
    pub re_pattern: Regex,
    pub quotes: Vec<(char, char)>,
    pub key_value_delimiter: String,
}

impl std::fmt::Display for KeyValueFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "keyvalue(\"{:?}\", \"{:?}\")",
            self.re_pattern, self.quotes
        )
    }
}

pub fn apply_filter(value: &Value, filter: &KeyValueFilter) -> Result<Value, GrokRuntimeError> {
    match value {
        Value::Bytes(bytes) => {
            let mut result = Value::Object(BTreeMap::default());
            let value = String::from_utf8_lossy(bytes);
            filter
                .re_pattern
                .captures_iter(value.as_ref())
                .for_each(|c| {
                    parse_key_value_capture(filter, &mut result, c);
                });
            Ok(result)
        }
        _ => Err(GrokRuntimeError::FailedToApplyFilter(
            filter.to_string(),
            value.to_string(),
        )),
    }
}

fn parse_key_value_capture(filter: &KeyValueFilter, result: &mut Value, c: Result<Captures, fancy_regex::Error>) {
    let key = parse_key(extract_capture(&c, 1), filter.quotes.as_slice());
    if !key.contains(' ') {
        let value = extract_capture(&c, 2);
        // trim trailing comma for value
        let value = value.trim_end_matches(|c| c == ',');

        if let Ok((_, value)) = parse_value(value, filter.quotes.as_slice()) {
            if !(value.is_null()
                || matches!(&value, Value::Bytes(b) if b.is_empty())
                || key.is_empty())
            {
                let path = crate::path!(key);
                match result.get(path).cloned() {
                    Some(Value::Array(mut values)) => {
                        values.push(value);
                        result.insert(path, values);
                    }
                    Some(prev) => {
                        result.insert(path, Value::Array(vec![prev, value]));
                    }
                    None => {
                        result.insert(path, value);
                    }
                };
            }
        }
    }
}

fn extract_capture<'a>(c: &'a Result<Captures<'a>, fancy_regex::Error>, i: usize) -> &'a str {
    c.as_ref()
        .map(|c| c.get(i).map(|m| m.as_str()))
        .unwrap_or_default()
        .unwrap_or_default()
        .trim()
}

type SResult<'a, O> = IResult<&'a str, O, (&'a str, nom::error::ErrorKind)>;

/// Parses quoted strings.
#[inline]
fn parse_quoted(quotes: &(char, char)) -> impl Fn(&str) -> SResult<&str> + '_ {
    move |input| {
        delimited(
            char(quotes.0),
            map(opt(take_while1(|c: char| c != quotes.1)), |inner| {
                inner.unwrap_or("")
            }),
            char(quotes.1),
        )(input)
    }
}

#[inline]
fn quoted(quotes: &[(char, char)]) -> impl Fn(&str) -> SResult<&str> + '_ {
    move |input| {
        let mut last_err = None;
        for quotes in quotes {
            match parse_quoted(quotes)(input) {
                done @ Ok(..) => return done,
                err @ Err(..) => last_err = Some(err), // continue
            }
        }
        last_err.unwrap()
    }
}

/// Parses the value.
/// The value has two parsing strategies.
///
/// 1. The value is quoted - parse until the end quote
/// 2. Otherwise, we parse until regex matches
#[inline]
fn parse_value<'a>(input: &'a str, quotes: &'a [(char, char)]) -> SResult<'a, Value> {
    alt((
        parse_null,
        parse_boolean,
        parse_number,
        quoted(quotes).and_then(parse_string),
        parse_string,
    ))(input)
}

fn parse_string(input: &str) -> SResult<Value> {
    map(rest, |s: &str| {
        Value::Bytes(Bytes::copy_from_slice(s.trim().as_bytes()))
    })(input)
}

fn parse_number(input: &str) -> SResult<Value> {
    let res = map(terminated(double, eof), |v| {
        if ((v as i64) as f64 - v).abs() == 0.0 {
            // can be safely converted to Integer without precision loss
            Value::Integer(v as i64)
        } else {
            Value::Float(NotNan::new(v).expect("not a float"))
        }
    })(input)
    .map_err(|e| match e {
        // double might return Failure(an unrecoverable error) - make it recoverable
        nom::Err::Failure(_) => nom::Err::Error((input, nom::error::ErrorKind::Float)),
        e => e,
    });
    match res {
        // check if it is a valid octal number(start with 0) - keep parsed as a decimal though
        Ok((_, Value::Integer(_)))
            if input.starts_with('0') && input.contains(|c| c == '8' || c == '9') =>
        {
            Err(nom::Err::Error((input, nom::error::ErrorKind::OctDigit)))
        }
        res => res,
    }
}

fn parse_null(input: &str) -> SResult<Value> {
    value(Value::Null, tag("null"))(input)
}

fn parse_boolean(input: &str) -> SResult<Value> {
    let parse_true = value(Value::Boolean(true), tag("true"));
    let parse_false = value(Value::Boolean(false), tag("false"));

    alt((parse_true, parse_false))(input)
}

/// Removes quotes from the key if needed.
fn parse_key<'a>(input: &'a str, quotes: &'a [(char, char)]) -> &'a str {
    quoted(quotes)(input)
        .map(|(_, key)| key)
        .unwrap_or_else(|_| input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key() {
        assert_eq!("key", parse_key("key", DEFAULT_QUOTES));
        assert_eq!("key", parse_key(r#""key""#, DEFAULT_QUOTES));
        assert_eq!("key", parse_key(r#"#key#"#, &[('#', '#')]));
    }

    #[test]
    fn test_parse_value() {
        assert_eq!(
            Ok(("", Value::from("value"))),
            parse_value("value", DEFAULT_QUOTES)
        );
        // trim whitespaces
        assert_eq!(
            Ok(("", Value::from("value"))),
            parse_value(" value ", DEFAULT_QUOTES)
        );
        // remove quotes
        assert_eq!(
            Ok(("", Value::from("value"))),
            parse_value(r#""value""#, DEFAULT_QUOTES)
        );
        // remove non-default quotes
        assert_eq!(
            Ok(("", Value::from("value"))),
            parse_value(r#"#value#"#, &[('#', '#')])
        );
        assert_eq!(
            Ok(("", Value::Null)),
            parse_value(r#"null"#, DEFAULT_QUOTES)
        );
        assert_eq!(
            Ok(("", Value::from(true))),
            parse_value(r#"true"#, DEFAULT_QUOTES)
        );
        assert_eq!(
            Ok(("", Value::from(false))),
            parse_value(r#"false"#, DEFAULT_QUOTES)
        );
        assert_eq!(
            Ok(("", Value::from(12))),
            parse_value(r#"12"#, DEFAULT_QUOTES)
        );
        assert_eq!(
            Ok(("", Value::from(1.2))),
            parse_value(r#"1.2"#, DEFAULT_QUOTES)
        );
    }
}
