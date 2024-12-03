use crate::compiler::prelude::*;
use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while, take_while1},
    character::complete::{char, digit1, satisfy},
    combinator::{cut, map, opt, recognize, value},
    error::{context, ContextError, FromExternalError, ParseError},
    multi::{many1, separated_list0},
    number::complete::double,
    sequence::{preceded, separated_pair, terminated, tuple},
    AsChar, IResult, InputTakeAtPosition,
};
use std::num::ParseIntError;

pub(crate) fn parse_ruby_hash(input: &str) -> ExpressionResult<Value> {
    let result = parse_hash(input)
        .map_err(|err| match err {
            nom::Err::Error(err) | nom::Err::Failure(err) => {
                // Create a descriptive error message if possible.
                nom::error::convert_error(input, err)
            }
            nom::Err::Incomplete(_) => err.to_string(),
        })
        .and_then(|(rest, result)| {
            rest.trim()
                .is_empty()
                .then_some(result)
                .ok_or_else(|| "could not parse whole line successfully".into())
        })?;

    Ok(result)
}

trait HashParseError<T>: ParseError<T> + ContextError<T> + FromExternalError<T, ParseIntError> {}
impl<T, E: ParseError<T> + ContextError<T> + FromExternalError<T, ParseIntError>> HashParseError<T>
    for E
{
}

fn sp<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";

    take_while(move |c| chars.contains(c))(input)
}

fn parse_inner_str<'a, E: ParseError<&'a str>>(
    delimiter: char,
) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str, E> {
    move |input| {
        map(
            opt(escaped(
                recognize(many1(tuple((
                    take_while1(|c: char| c != '\\' && c != delimiter),
                    // Consume \something
                    opt(tuple((
                        satisfy(|c| c == '\\'),
                        satisfy(|c| c != '\\' && c != delimiter),
                    ))),
                )))),
                '\\',
                satisfy(|c| c == '\\' || c == delimiter),
            )),
            |inner| inner.unwrap_or(""),
        )(input)
    }
}

/// Parses text with a given delimiter.
fn parse_str<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    delimiter: char,
) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str, E> {
    context(
        "string",
        preceded(
            char(delimiter),
            cut(terminated(parse_inner_str(delimiter), char(delimiter))),
        ),
    )
}

fn parse_boolean<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, bool, E> {
    let parse_true = value(true, tag("true"));
    let parse_false = value(false, tag("false"));

    alt((parse_true, parse_false))(input)
}

fn parse_nil<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Value, E> {
    value(Value::Null, tag("nil"))(input)
}

fn parse_bytes<'a, E: HashParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Bytes, E> {
    context(
        "bytes",
        map(alt((parse_str('"'), parse_str('\''))), |value| {
            Bytes::copy_from_slice(value.as_bytes())
        }),
    )(input)
}

fn parse_symbol_key<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    take_while1(move |item: <T as InputTakeAtPosition>::Item| {
        let c = item.as_char();
        c.is_alphanum() || c == '_'
    })(input)
}

fn parse_colon_key<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, KeyString, E> {
    map(
        preceded(
            char(':'),
            alt((parse_str('"'), parse_str('\''), parse_symbol_key)),
        ),
        KeyString::from,
    )(input)
}

// This parse_key function allows some cases that shouldn't be produced by ruby.
// For example, { foo => "bar" } shouldn't be parsed but { foo: "bar" } should.
// Considering that Vector's goal is to parse log produced by other applications
// and that Vector is NOT a ruby parser, cases like the following one are ignored
// because they shouldn't appear in the logs.
// That being said, handling all the corner cases from Ruby's syntax would imply
// increasing a lot the code complexity which is probably not necessary considering
// that Vector is not a Ruby parser.
fn parse_key<'a, E: HashParseError<&'a str>>(input: &'a str) -> IResult<&'a str, KeyString, E> {
    alt((
        map(
            alt((parse_str('"'), parse_str('\''), parse_symbol_key, digit1)),
            KeyString::from,
        ),
        parse_colon_key,
    ))(input)
}

fn parse_array<'a, E: HashParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Value, E> {
    context(
        "array",
        map(
            preceded(
                char('['),
                cut(terminated(
                    separated_list0(preceded(sp, char(',')), parse_value),
                    preceded(sp, char(']')),
                )),
            ),
            Value::Array,
        ),
    )(input)
}

fn parse_key_value<'a, E: HashParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (KeyString, Value), E> {
    separated_pair(
        preceded(sp, parse_key),
        cut(preceded(sp, alt((tag(":"), tag("=>"))))),
        parse_value,
    )(input)
}

fn parse_hash<'a, E: HashParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Value, E> {
    context(
        "map",
        map(
            preceded(
                char('{'),
                cut(terminated(
                    map(
                        separated_list0(preceded(sp, char(',')), parse_key_value),
                        |tuple_vec| tuple_vec.into_iter().collect(),
                    ),
                    preceded(sp, char('}')),
                )),
            ),
            Value::Object,
        ),
    )(input)
}

fn parse_value<'a, E: HashParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Value, E> {
    preceded(
        sp,
        alt((
            parse_nil,
            parse_hash,
            parse_array,
            map(parse_colon_key, Value::from),
            map(parse_bytes, Value::Bytes),
            map(double, |value| Value::Float(NotNan::new(value).unwrap())),
            map(parse_boolean, Value::Boolean),
        )),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_object() {
        let result = parse_ruby_hash("{}").unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn test_parse_arrow_empty_array() {
        parse_ruby_hash("{ :array => [] }").unwrap();
    }

    #[test]
    fn test_parse_symbol_key() {
        let result = parse_ruby_hash(r#"{ :key => "foo", :number => 500 }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("key").unwrap();
        assert!(value.is_bytes());
        assert_eq!(value.as_bytes().unwrap(), "foo");
        assert!(result.get("number").unwrap().is_float());
    }

    #[test]
    fn test_parse_symbol_colon_separator() {
        let result = parse_ruby_hash(r#"{ key: "foo" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("key").unwrap();
        assert!(value.is_bytes());
        assert_eq!(value.as_bytes().unwrap(), "foo");
    }

    #[test]
    fn test_parse_arrow_object() {
        let result = parse_ruby_hash(
            r#"{ "hello" => "world", "number" => 42, "float" => 4.2, "array" => [1, 2.3], "object" => { "nope" => nil } }"#,
        )
            .unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("hello").unwrap().is_bytes());
        assert!(result.get("number").unwrap().is_float());
        assert!(result.get("float").unwrap().is_float());
        assert!(result.get("array").unwrap().is_array());
        assert!(result.get("object").unwrap().is_object());
        let child = result.get("object").unwrap().as_object().unwrap();
        assert!(child.get("nope").unwrap().is_null());
    }

    #[test]
    fn test_parse_arrow_object_key_number() {
        let result = parse_ruby_hash(r#"{ 42 => "hello world" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("42").unwrap().is_bytes());
    }

    #[test]
    fn test_parse_arrow_object_key_colon() {
        let result = parse_ruby_hash(
            r#"{ :colon => "hello world", :"double" => "quote", :'simple' => "quote" }"#,
        )
        .unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("colon").unwrap().is_bytes());
        assert!(result.get("double").unwrap().is_bytes());
        assert!(result.get("simple").unwrap().is_bytes());
    }

    #[test]
    fn test_parse_arrow_object_key_underscore() {
        let result = parse_ruby_hash(r#"{ :with_underscore => "hello world" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("with_underscore").unwrap().is_bytes());
    }

    #[test]
    fn test_parse_colon_object_double_quote() {
        let result = parse_ruby_hash(r#"{ "hello": "world" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("hello").unwrap();
        assert_eq!(value, &Value::Bytes("world".into()));
    }

    #[test]
    fn test_parse_colon_object_single_quote() {
        let result = parse_ruby_hash("{ 'hello': 'world' }").unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("hello").unwrap();
        assert_eq!(value, &Value::Bytes("world".into()));
    }

    #[test]
    fn test_parse_colon_object_no_quote() {
        let result = parse_ruby_hash(r#"{ hello: "world" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("hello").unwrap();
        assert_eq!(value, &Value::Bytes("world".into()));
    }

    #[test]
    fn test_parse_dash() {
        let result = parse_ruby_hash(r#"{ "with-dash" => "foo" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("with-dash").unwrap().is_bytes());
    }

    #[test]
    fn test_parse_quote() {
        let result = parse_ruby_hash(r#"{ "with'quote" => "and\"double\"quote" }"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        let value = result.get("with'quote").unwrap();
        assert_eq!(value, &Value::Bytes("and\\\"double\\\"quote".into()));
    }

    #[test]
    fn test_parse_weird_format() {
        let result =
            parse_ruby_hash(r#"{:hello=>"world",'number'=>42,"weird"=>'format\'here'}"#).unwrap();
        assert!(result.is_object());
        let result = result.as_object().unwrap();
        assert!(result.get("hello").unwrap().is_bytes());
        assert!(result.get("number").unwrap().is_float());
    }

    #[test]
    fn test_non_hash() {
        assert!(parse_ruby_hash(r#""hello world""#).is_err());
    }
}
