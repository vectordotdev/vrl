use crate::compiler::prelude::*;
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
    str::FromStr,
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

        let redactor = Redactor::Full;

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
                        Pattern::Regex(regex) => regex
                            .replace_all(&input, redactor.pattern())
                            .into_owned()
                            .into(),
                        Pattern::String(pattern) => {
                            input.replace(pattern, redactor.pattern()).into()
                        }
                    })
            }
            Filter::UsSocialSecurityNumber => {
                US_SOCIAL_SECURITY_NUMBER.replace_all(input, redactor.pattern())
            }
        }
    }
}

/// The recipe for redacting the matched filters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Redactor {
    #[default]
    Full,
}

impl Redactor {
    fn pattern(&self) -> &str {
        use Redactor::Full;

        match self {
            Full => "[REDACTED]",
        }
    }
}

impl FromStr for Redactor {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use Redactor::Full;

        match s {
            "full" => Ok(Full),
            _ => Err("unknown redactor"),
        }
    }
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
