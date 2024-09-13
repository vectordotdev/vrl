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
    combinator::{map, map_res, opt, rest, value},
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
    let filter = KeyValueFilter::from_args(f.args.as_deref().unwrap_or_default().iter())
        .ok_or_else(|| GrokStaticError::InvalidFunctionArguments(f.name.clone()))?;
    Ok(GrokFilter::KeyValue(filter))
}

#[derive(Debug, Clone)]
pub struct KeyValueFilter {
    pub re_pattern: Regex,
    pub quotes: Vec<(char, char)>,
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

impl KeyValueFilter {
    fn from_args<'a>(mut args: impl Iterator<Item = &'a FunctionArgument>) -> Option<Self> {
        let key_value_delimiter = match args.next() {
            Some(FunctionArgument::Arg(Value::Bytes(ref bytes))) => &String::from_utf8_lossy(bytes),
            Some(_) => return None,
            None => DEFAULT_KEYVALUE_DELIMITER,
        };

        let value_re = match args.next() {
            Some(FunctionArgument::Arg(Value::Bytes(ref bytes))) => {
                [DEFAULT_VALUE_RE, &String::from_utf8_lossy(bytes)].concat()
            }
            Some(_) => return None,
            // default allowed unescaped symbols
            None => DEFAULT_VALUE_RE.to_string(),
        };

        let quotes = parse_quotes(args.next())?;
        let field_delimiters = parse_field_delimiters(args.next())?;

        Some(Self {
            re_pattern: regex_from_config(
                key_value_delimiter,
                &value_re,
                quotes.clone(),
                field_delimiters,
            )?,
            quotes,
        })
    }
}

fn parse_quotes(arg: Option<&FunctionArgument>) -> Option<Vec<(char, char)>> {
    match arg {
        Some(FunctionArgument::Arg(Value::Bytes(ref bytes))) => {
            let pair = String::from_utf8_lossy(bytes);
            match pair {
                pair if pair.len() == 2 => {
                    let mut chars = pair.chars();
                    Some(vec![(
                        chars.next().expect("open quote"),
                        chars.next().expect("closing quote"),
                    )])
                }
                pair if pair.is_empty() => Some(Vec::from(DEFAULT_QUOTES)),
                _ => None,
            }
        }
        Some(_) => None,
        None => Some(Vec::from(DEFAULT_QUOTES)),
    }
}

fn parse_field_delimiters(arg: Option<&FunctionArgument>) -> Option<(String, String)> {
    match arg {
        Some(FunctionArgument::Arg(Value::Bytes(ref bytes))) => {
            let delimiter_str = String::from_utf8_lossy(bytes);
            let mut chars = delimiter_str.chars();
            match (chars.next(), chars.next(), chars.as_str()) {
                (Some(single), None, _) => Some((single.to_string(), single.to_string())),
                (Some(left), Some(right), "") => Some((left.to_string(), right.to_string())),
                _ => None,
            }
        }
        Some(_) => None,
        None => Some((
            DEFAULT_DELIMITERS.0.to_string(),
            DEFAULT_DELIMITERS.1.to_string(),
        )),
    }
}

pub fn regex_from_config(
    key_value_delimiter: &str,
    value_re: &str,
    quotes: Vec<(char, char)>,
    field_delimiters: (String, String),
) -> Option<Regex> {
    // start group
    let mut quoting = String::from("(");
    // add quotes with OR
    for (left, right) in quotes {
        let left = left.to_string();
        let right = right.to_string();
        quoting.extend([&left, "[^", &left, "]+", &right, "|"]);
    }

    quoting.push('[');

    let keyvalue = [
        "(?<=[",
        &field_delimiters.0,
        "]|^)",
        // key
        quoting.as_str(),
        value_re,
        "]+)",
        // delimiter
        key_value_delimiter,
        // value
        quoting.as_str(),
        value_re,
        "]+)",
        "(?:[",
        &field_delimiters.1,
        "]|$)",
    ]
    .concat();

    Regex::new(keyvalue.as_str()).ok()
}

impl KeyValueFilter {
    pub fn apply_filter(&self, value: &Value) -> Result<Value, GrokRuntimeError> {
        match value {
            Value::Bytes(bytes) => {
                let mut result = Value::Object(BTreeMap::default());
                let value = String::from_utf8_lossy(bytes);
                self.re_pattern.captures_iter(value.as_ref()).for_each(|c| {
                    self.parse_key_value_capture(&mut result, c);
                });
                Ok(result)
            }
            _ => Err(GrokRuntimeError::FailedToApplyFilter(
                self.to_string(),
                value.to_string(),
            )),
        }
    }

    fn parse_key_value_capture(&self, result: &mut Value, c: Result<Captures, fancy_regex::Error>) {
        let key = parse_key(extract_capture(&c, 1), &self.quotes);
        if !key.contains(' ') {
            let value = extract_capture(&c, 2);
            // trim trailing comma for value
            let value = value.trim_end_matches(|c| c == ',');

            if let Ok((_, value)) = parse_value(value, &self.quotes) {
                if !(value.is_null()
                    || matches!(&value, Value::Bytes(b) if b.is_empty())
                    || key.is_empty())
                {
                    let path = crate::path!(key);
                    match result.get_mut(path) {
                        Some(Value::Array(ref mut values)) => values.push(value),
                        Some(prev) => {
                            // Replace existing non-array values with an array containing that value
                            // followed by the new one. We can't just put that old value into the
                            // array directly because we only have a `mut` reference to it, hence
                            // the need `replace` it first.
                            let old_value = std::mem::replace(prev, Value::Null);
                            *prev = Value::Array(vec![old_value, value]);
                        }
                        None => {
                            result.insert(path, value);
                        }
                    }
                }
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
    map_res(terminated(double, eof), |v| {
        // can be safely converted to Integer without precision loss
        if ((v as i64) as f64 - v).abs() == 0.0 {
            // Check if it is a valid octal number(start with 0) - keep parsed as a decimal though.
            if input.starts_with('0') && input.contains(|c| c == '8' || c == '9') {
                Err(nom::Err::Error((input, nom::error::ErrorKind::OctDigit)))
            } else {
                Ok(Value::Integer(v as i64))
            }
        } else {
            Ok(Value::Float(NotNan::new(v).expect("not a float")))
        }
    })(input)
    .map_err(|e| match e {
        // double might return Failure(an unrecoverable error) - make it recoverable
        nom::Err::Failure(_) => nom::Err::Error((input, nom::error::ErrorKind::Float)),
        e => e,
    })
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
