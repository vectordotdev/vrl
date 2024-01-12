use crate::compiler::prelude::*;
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
};

// https://www.oreilly.com/library/view/regular-expressions-cookbook/9781449327453/ch04s12.html
// (converted to non-lookaround version given `regex` does not support lookarounds)
// See also: https://www.ssa.gov/history/ssn/geocard.html
static US_SOCIAL_SECURITY_NUMBER: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(
    r#"(?x)                                                               # Ignore whitespace and comments in the regex expression.
    (?:00[1-9]|0[1-9][0-9]|[1-578][0-9]{2}|6[0-57-9][0-9]|66[0-57-9])-    # Area number: 001-899 except 666
    (?:0[1-9]|[1-9]0|[1-9][1-9])-                                         # Group number: 01-99
    (?:000[1-9]|00[1-9]0|0[1-9]00|[1-9]000|[1-9]{4})                      # Serial number: 0001-9999
    "#).unwrap()
});

#[derive(Clone, Copy, Debug)]
pub struct Redact;

impl Function for Redact {
    fn identifier(&self) -> &'static str {
        "redact"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES | kind::OBJECT | kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "filters",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "redactor",
                kind: kind::OBJECT | kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "regex",
                source: r#"redact("my id is 123456", filters: [r'\d+'])"#,
                result: Ok(r#"my id is [REDACTED]"#),
            },
            Example {
                title: "us_social_security_number",
                source: r#"redact({ "name": "John Doe", "ssn": "123-12-1234"}, filters: ["us_social_security_number"])"#,
                result: Ok(r#"{ "name": "John Doe", "ssn": "[REDACTED]" }"#),
            },
            // TODO: redactor examples
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        let filters = arguments
            .required_array("filters")?
            .into_iter()
            .map(|expr| {
                expr.resolve_constant(state)
                    .ok_or(function::Error::ExpectedStaticExpression {
                        keyword: "filters",
                        expr,
                    })
            })
            .map(|value| {
                value.and_then(|value| {
                    value
                        .clone()
                        .try_into()
                        .map_err(|error| function::Error::InvalidArgument {
                            keyword: "filters",
                            value,
                            error,
                        })
                })
            })
            .collect::<std::result::Result<Vec<Filter>, _>>()?;

        let redactor = arguments
            .optional_literal("redactor", state)?
            .map(|value| {
                value
                    .clone()
                    .try_into()
                    .map_err(|error| function::Error::InvalidArgument {
                        keyword: "redactor",
                        value,
                        error,
                    })
            })
            .transpose()?
            .unwrap_or(Redactor::Full);

        Ok(RedactFn {
            value,
            filters,
            redactor,
        }
        .as_expr())
    }
}

//-----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct RedactFn {
    value: Box<dyn Expression>,
    filters: Vec<Filter>,
    redactor: Redactor,
}

fn redact(value: Value, filters: &[Filter], redactor: &Redactor) -> Value {
    // possible optimization. match the redactor here, and use different calls depending on
    // the value, so that we don't have to do the comparision in the loop of replacment.
    // that would complicate the code though.
    match value {
        Value::Bytes(bytes) => {
            let input = String::from_utf8_lossy(&bytes);
            let output = filters.iter().fold(input, |input, filter| {
                filter.redact(&input, redactor).into_owned().into()
            });
            Value::Bytes(output.into_owned().into())
        }
        Value::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| redact(value, filters, redactor))
                .collect();
            Value::Array(values)
        }
        Value::Object(map) => {
            let map = map
                .into_iter()
                .map(|(key, value)| (key, redact(value, filters, redactor)))
                .collect();
            Value::Object(map)
        }
        _ => value,
    }
}

impl FunctionExpression for RedactFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let filters = &self.filters;
        let redactor = &self.redactor;

        Ok(redact(value, filters, redactor))
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        self.value.type_def(state).infallible()
    }
}

//-----------------------------------------------------------------------------

/// The redaction filter to apply to the given value.
#[derive(Debug, Clone)]
enum Filter {
    Pattern(Vec<Pattern>),
    UsSocialSecurityNumber,
}

#[derive(Debug, Clone)]
enum Pattern {
    Regex(regex::Regex),
    String(String),
}

impl TryFrom<Value> for Filter {
    type Error = &'static str;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Object(object) => {
                let r#type = match object
                    .get("type")
                    .ok_or("filters specified as objects must have type parameter")?
                {
                    Value::Bytes(bytes) => Ok(bytes.clone()),
                    _ => Err("type key in filters must be a string"),
                }?;

                match r#type.as_ref() {
                    b"us_social_security_number" => Ok(Filter::UsSocialSecurityNumber),
                    b"pattern" => {
                        let patterns = match object
                            .get("patterns")
                            .ok_or("pattern filter must have `patterns` specified")?
                        {
                            Value::Array(array) => Ok(array
                                .iter()
                                .map(|value| match value {
                                    Value::Regex(regex) => Ok(Pattern::Regex((**regex).clone())),
                                    Value::Bytes(bytes) => Ok(Pattern::String(
                                        String::from_utf8_lossy(bytes).into_owned(),
                                    )),
                                    _ => Err("`patterns` must be regular expressions"),
                                })
                                .collect::<std::result::Result<Vec<_>, _>>()?),
                            _ => Err("`patterns` must be array of regular expression literals"),
                        }?;
                        Ok(Filter::Pattern(patterns))
                    }
                    _ => Err("unknown filter name"),
                }
            }
            Value::Bytes(bytes) => match bytes.as_ref() {
                b"pattern" => Err("pattern cannot be used without arguments"),
                b"us_social_security_number" => Ok(Filter::UsSocialSecurityNumber),
                _ => Err("unknown filter name"),
            },
            Value::Regex(regex) => Ok(Filter::Pattern(vec![Pattern::Regex((*regex).clone())])),
            _ => Err("unknown literal for filter, must be a regex, filter name, or object"),
        }
    }
}

impl Filter {
    fn redact<'t>(&self, input: &'t str, redactor: &Redactor) -> Cow<'t, str> {
        match &self {
            Filter::Pattern(patterns) => {
                patterns
                    .iter()
                    .fold(Cow::Borrowed(input), |input, pattern| match pattern {
                        Pattern::Regex(regex) => {
                            regex.replace_all(&input, redactor).into_owned().into()
                        }
                        Pattern::String(pattern) => str_replace(&input, pattern, redactor).into(),
                    })
            }
            Filter::UsSocialSecurityNumber => {
                US_SOCIAL_SECURITY_NUMBER.replace_all(input, redactor)
            }
        }
    }
}

fn str_replace(haystack: &str, pattern: &str, redactor: &Redactor) -> String {
    let mut result = String::new();
    let mut last_end = 0;
    for (start, original) in haystack.match_indices(pattern) {
        result.push_str(&haystack[last_end..start]);
        redactor.replace_str(original, &mut result);
        last_end = start + original.len();
    }
    result.push_str(&haystack[last_end..]);
    result
}

/// The recipe for redacting the matched filters.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum Redactor {
    #[default]
    Full,
    /// Replace with a fixed string
    Text(String), // possible optimization: use Arc<str> instead of String to speed up cloning
    // this simplifies the code, but the Debug implmentation probably isn't very useful
    // alternatively we could have a separate variant for each hash algorithm/variant combination
    // we could also create a custom Debug implementation that does a comparison of the fn pointer
    // to function pointers we might use.
    /// Replace with a hash of the redacted content
    Hash(fn(&[u8]) -> String),
}

const REDACTED: &str = "[REDACTED]";

impl Redactor {
    fn replace_str(&self, original: &str, dst: &mut String) {
        match self {
            Redactor::Full => {
                dst.push_str(REDACTED);
            }
            Redactor::Text(s) => {
                dst.push_str(s);
            }
            Redactor::Hash(hash) => dst.push_str(&hash(original.as_bytes())),
        }
    }

    fn from_object(obj: ObjectMap) -> std::result::Result<Self, &'static str> {
        let r#type = match obj.get("type").ok_or(
            "redactor specified as objects must have type
        parameter",
        )? {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            _ => Err("type key in redactor must be a string"),
        }?;

        match r#type.as_ref() {
            b"full" => Ok(Redactor::Full),
            b"text" => {
                match obj.get("replacement").ok_or(
                    "text redactor must have
                `replacement` specified",
                )? {
                    Value::Bytes(bytes) => {
                        Ok(Redactor::Text(String::from_utf8_lossy(bytes).into_owned()))
                    }
                    _ => Err("`replacement` must be a string"),
                }
            }
            b"sha2" => {
                let hash = if let Some(variant) = obj.get("variant") {
                    match variant
                        .as_bytes()
                        .ok_or("`variant` must be a string")?
                        .as_ref()
                    {
                        b"SHA-224" => hex_digest::<sha_2::Sha224>,
                        b"SHA-256" => hex_digest::<sha_2::Sha256>,
                        b"SHA-384" => hex_digest::<sha_2::Sha384>,
                        b"SHA-512" => hex_digest::<sha_2::Sha512>,
                        b"SHA-512/224" => hex_digest::<sha_2::Sha512_224>,
                        b"SHA-512/256" => hex_digest::<sha_2::Sha512_256>,
                        _ => return Err("invalid sha2 variant"),
                    }
                } else {
                    hex_digest::<sha_2::Sha512_256>
                };
                Ok(Redactor::Hash(hash))
            }
            b"sha3" => {
                let hash = if let Some(variant) = obj.get("variant") {
                    match variant
                        .as_bytes()
                        .ok_or("`variant must be a string")?
                        .as_ref()
                    {
                        b"SHA3-224" => hex_digest::<sha_3::Sha3_224>,
                        b"SHA3-256" => hex_digest::<sha_3::Sha3_256>,
                        b"SHA3-384" => hex_digest::<sha_3::Sha3_384>,
                        b"SHA3-512" => hex_digest::<sha_3::Sha3_512>,
                        _ => return Err("invalid sha2 variant"),
                    }
                } else {
                    hex_digest::<sha_2::Sha512_256>
                };
                Ok(Redactor::Hash(hash))
            }
            _ => Err("unknown `type` for `redactor`"),
        }
    }
}

impl regex::Replacer for &Redactor {
    fn replace_append(&mut self, caps: &regex::Captures, dst: &mut String) {
        self.replace_str(&caps[0], dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<str>> {
        match self {
            Redactor::Full => Some(REDACTED.into()),
            Redactor::Text(s) => Some(s.into()),
            _ => None,
        }
    }
}

impl TryFrom<Value> for Redactor {
    type Error = &'static str;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Object(object) => Redactor::from_object(object),
            Value::Bytes(bytes) => match bytes.as_ref() {
                b"full" => Ok(Redactor::Full),
                b"sha2" => Ok(Redactor::Hash(hex_digest::<sha_2::Sha512_256>)),
                b"sha3" => Ok(Redactor::Hash(hex_digest::<sha_3::Sha3_512>)),
                _ => Err("unknown name of redactor"),
            },
            _ => Err("unknown literal for redactor, must be redactor name or object"),
        }
    }
}

/// Compute the digest of some bytes as hex string
fn hex_digest<T: digest::Digest>(value: &[u8]) -> String {
    hex::encode(T::digest(value))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;
    use regex::Regex;

    test_function![
        redact => Redact;

        regex {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec![Regex::new(r"\d+").unwrap()],
             ],
             want: Ok("hello [REDACTED] world"),
             tdef: TypeDef::bytes().infallible(),
        }

        patterns {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec![
                     value!({
                         "type": "pattern",
                         "patterns": ["123456"]
                     })
                 ],
             ],
             want: Ok("hello [REDACTED] world"),
             tdef: TypeDef::bytes().infallible(),
        }

        us_social_security_number{
             args: func_args![
                 value: "hello 123-12-1234 world",
                 filters: vec!["us_social_security_number"],
             ],
             want: Ok("hello [REDACTED] world"),
             tdef: TypeDef::bytes().infallible(),
        }

        invalid_filter {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec!["not a filter"],
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }

        missing_patterns {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec![
                     value!({
                         "type": "pattern",
                     })
                 ],
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }
    ];
}
