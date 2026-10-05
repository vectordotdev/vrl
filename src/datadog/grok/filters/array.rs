use std::convert::TryFrom;

use crate::value::Value;
use bytes::Bytes;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take, take_until},
    combinator::map,
    multi::separated_list0,
    sequence::{preceded, terminated},
};

use super::super::{
    ast::{Function, FunctionArgument},
    grok_filter::GrokFilter,
    parse_grok_rules::Error as GrokStaticError,
};

pub fn filter_from_function(f: &Function) -> Result<GrokFilter, GrokStaticError> {
    let args = f.args.as_ref();
    let args_len = args.map_or(0, std::vec::Vec::len);
    let invalid = || GrokStaticError::InvalidFunctionArguments(f.name.clone());

    let mut delimiter = None;
    let mut value_filter = None;
    let mut brackets = None;
    if args_len == 1 {
        match &args.unwrap()[0] {
            FunctionArgument::Function(f) => value_filter = Some(GrokFilter::try_from(f)?),
            arg @ FunctionArgument::Arg(_) => {
                delimiter = Some(arg.to_utf8_lossy().ok_or_else(invalid)?);
            }
        }
    } else if args_len == 2 {
        let args = args.unwrap();
        match (args[0].to_utf8_lossy(), args[1].to_utf8_lossy(), &args[1]) {
            (Some(left), Some(right), _) => {
                brackets = Some(left);
                delimiter = Some(right);
            }
            (Some(delim), None, FunctionArgument::Function(func)) => {
                delimiter = Some(delim);
                value_filter = Some(GrokFilter::try_from(func)?);
            }
            _ => return Err(invalid()),
        }
    } else if args_len == 3 {
        let args = args.unwrap();
        match (args[0].to_utf8_lossy(), args[1].to_utf8_lossy(), &args[2]) {
            (Some(left), Some(right), FunctionArgument::Function(func)) => {
                brackets = Some(left);
                delimiter = Some(right);
                value_filter = Some(GrokFilter::try_from(func)?);
            }
            _ => return Err(invalid()),
        }
    } else if args_len > 3 {
        return Err(invalid());
    }

    let brackets = match &brackets {
        None => None,
        Some(b) if b.is_empty() => Some(("", "")),
        Some(b) if b.len() == 1 => {
            let char = &b[..1];
            Some((char, char))
        }
        Some(b) if b.len() == 2 => Some((&b[..1], &b[1..2])),
        _ => {
            return Err(invalid());
        }
    };

    Ok(GrokFilter::Array(
        brackets.map(|(start, end)| (start.to_string(), end.to_string())),
        delimiter,
        Box::new(value_filter),
    ))
}

type SResult<'a, O> = IResult<&'a str, O, (&'a str, nom::error::ErrorKind)>;

pub fn parse<'a>(
    input: &'a str,
    brackets: Option<(&'a str, &'a str)>,
    delimiter: Option<&'a str>,
) -> Result<Vec<Value>, String> {
    let result = parse_array(brackets, delimiter)(input)
        .map_err(|_| format!("could not parse '{input}' as array"))
        .and_then(|(rest, result)| {
            rest.trim()
                .is_empty()
                .then_some(result)
                .ok_or_else(|| format!("could not parse '{input}' as array"))
        })?;

    Ok(result)
}

fn parse_array<'a>(
    brackets: Option<(&'a str, &'a str)>,
    delimiter: Option<&'a str>,
) -> impl Fn(&'a str) -> SResult<'a, Vec<Value>> {
    let brackets = brackets.unwrap_or(("[", "]"));
    let delimiter = delimiter.unwrap_or(",");
    move |input| {
        if brackets.0.is_empty() {
            // no enclosed brackets
            separated_list0(tag(delimiter), parse_value_no_brackets(delimiter)).parse(input)
        } else {
            preceded(
                tag(brackets.0),
                terminated(
                    separated_list0(tag(delimiter), parse_value(delimiter, brackets.1)),
                    tag(brackets.1),
                ),
            )
            .parse(input)
        }
    }
}

fn parse_value_no_brackets<'a>(delimiter: &'a str) -> impl Fn(&'a str) -> SResult<'a, Value> {
    move |input| {
        map(
            alt((take_until(delimiter), take(input.len()))),
            |value: &str| Value::Bytes(Bytes::copy_from_slice(value.as_bytes())),
        )
        .parse(input)
    }
}

fn parse_value<'a>(
    delimiter: &'a str,
    close_bracket: &'a str,
) -> impl Fn(&'a str) -> SResult<'a, Value> {
    move |input| {
        map(
            alt((take_until(delimiter), take_until(close_bracket))),
            |value: &str| Value::Bytes(Bytes::copy_from_slice(value.as_bytes())),
        )
        .parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default() {
        let result = parse("[ 1 ,2]", None, None).unwrap();
        assert_eq!(result, vec![" 1 ".into(), "2".into()]);
    }

    #[test]
    fn parses_with_non_default_brackets() {
        let result = parse("{1,2}", Some(("{", "}")), None).unwrap();
        assert_eq!(result, vec!["1".into(), "2".into()]);
    }

    #[test]
    fn parses_quotes() {
        let result = parse(r#"["1,2"]"#, None, None).unwrap();
        assert_eq!(result, vec!["\"1".into(), "2\"".into()]);
    }

    #[test]
    fn parses_escaped_special_characters() {
        let result = parse("[1\r2]", None, Some("\r")).unwrap();
        assert_eq!(result, vec!["1".into(), "2".into()]);

        let result = parse("[1\n2]", None, Some("\n")).unwrap();
        assert_eq!(result, vec!["1".into(), "2".into()]);

        let result = parse("[1\t2]", None, Some("\t")).unwrap();
        assert_eq!(result, vec!["1".into(), "2".into()]);
    }
}
