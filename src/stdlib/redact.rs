use crate::compiler::prelude::*;
use std::{
    borrow::Cow,
    convert::{TryFrom, TryInto},
    sync::LazyLock,
};

// https://www.oreilly.com/library/view/regular-expressions-cookbook/9781449327453/ch04s12.html
// (converted to non-lookaround version given `regex` does not support lookarounds)
// See also: https://www.ssa.gov/history/ssn/geocard.html
static US_SOCIAL_SECURITY_NUMBER: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        "(?x)                                                               # Ignore whitespace and comments in the regex expression.
    (?:00[1-9]|0[1-9][0-9]|[1-578][0-9]{2}|6[0-57-9][0-9]|66[0-57-9])-    # Area number: 001-899 except 666
    (?:0[1-9]|[1-9]0|[1-9][1-9])-                                         # Group number: 01-99
    (?:000[1-9]|00[1-9]0|0[1-9]00|[1-9]000|[1-9]{4})                      # Serial number: 0001-9999
    ").unwrap()
});

#[derive(Clone, Copy, Debug)]
pub struct Redact;

impl Function for Redact {
    fn identifier(&self) -> &'static str {
        "redact"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Redact sensitive data in `value` such as:

            - [US social security card numbers](https://www.ssa.gov/history/ssn/geocard.html)
            - Other forms of personally identifiable information with custom patterns

            This can help achieve compliance by ensuring sensitive data does not leave your network.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES | kind::OBJECT | kind::ARRAY,
                required: true,
                description: "The value to redact sensitive data from.

The function's behavior depends on `value`'s type:

- For strings, the sensitive data is redacted and a new string is returned.
- For arrays, the sensitive data is redacted in each string element.
- For objects, the sensitive data in each string value is masked, but the keys are not masked.

For arrays and objects, the function recurses into any nested arrays or objects. Any non-string elements are
skipped.

Redacted text is replaced with `[REDACTED]`.",
            },
            Parameter {
                keyword: "filters",
                kind: kind::ARRAY,
                required: true,
                description: "List of filters applied to `value`.

Each filter can be specified in the following ways:

- As a regular expression, which is used to redact text that match it.
- As an object with a `type` key that corresponds to a named filter and additional keys for customizing that filter.
- As a named filter, if it has no required parameters.

Named filters can be a:

- `pattern`: Redacts text matching any regular expressions specified in the `patterns`
	key, which is required. This is the expanded version of just passing a regular expression as a filter.
- `us_social_security_number`: Redacts US social security card numbers.

See examples for more details.

This parameter must be a static expression so that the argument can be validated at compile-time
to avoid runtime errors. You cannot use variables or other dynamic expressions with it.",
            },
            Parameter {
                keyword: "redactor",
                kind: kind::OBJECT | kind::BYTES,
                required: false,
                description: "Specifies what to replace the redacted strings with.

It is given as an object with a \"type\" key specifying the type of redactor to use
and additional keys depending on the type. The following types are supported:

- `full`: The default. Replace with the string \"[REDACTED]\".
- `text`: Replace with a custom string. The `replacement` key is required, and must
  contain the string that is used as a replacement.
- `sha2`: Hash the redacted text with SHA-2 as with [`sha2`](https://en.wikipedia.org/wiki/SHA-2). Supports two optional parameters:
	- `variant`: The variant of the algorithm to use. Defaults to SHA-512/256.
	- `encoding`: How to encode the hash as text. Can be base16 or base64.
		Defaults to base64.
- `sha3`: Hash the redacted text with SHA-3 as with [`sha3`](https://en.wikipedia.org/wiki/SHA-3). Supports two optional parameters:
	- `variant`: The variant of the algorithm to use. Defaults to SHA3-512.
	- `encoding`: How to encode the hash as text. Can be base16 or base64.
		Defaults to base64.


As a convenience you can use a string as a shorthand for common redactor patterns:

- `\"full\"` is equivalent to `{\"type\": \"full\"}`
- `\"sha2\"` is equivalent to `{\"type\": \"sha2\", \"variant\": \"SHA-512/256\", \"encoding\": \"base64\"}`
- `\"sha3\"` is equivalent to `{\"type\": \"sha3\", \"variant\": \"SHA3-512\", \"encoding\": \"base64\"}`

This parameter must be a static expression so that the argument can be validated at compile-time
to avoid runtime errors. You cannot use variables or other dynamic expressions with it.",
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Replace text using a regex",
                source: r#"redact("my id is 123456", filters: [r'\d+'])"#,
                result: Ok("my id is [REDACTED]"),
            },
            example! {
                title: "Replace us social security numbers in any field",
                source: r#"redact({ "name": "John Doe", "ssn": "123-12-1234"}, filters: ["us_social_security_number"])"#,
                result: Ok(r#"{ "name": "John Doe", "ssn": "[REDACTED]" }"#),
            },
            example! {
                title: "Replace with custom text",
                source: r#"redact("my id is 123456", filters: [r'\d+'], redactor: {"type": "text", "replacement": "***"})"#,
                result: Ok("my id is ***"),
            },
            example! {
                title: "Replace with SHA-2 hash",
                source: r#"redact("my id is 123456", filters: [r'\d+'], redactor: "sha2")"#,
                result: Ok("my id is GEtTedW1p6tC094dDKH+3B8P+xSnZz69AmpjaXRd63I="),
            },
            example! {
                title: "Replace with SHA-3 hash",
                source: r#"redact("my id is 123456", filters: [r'\d+'], redactor: "sha3")"#,
                result: Ok(
                    "my id is ZNCdmTDI7PeeUTFnpYjLdUObdizo+bIupZdl8yqnTKGdLx6X3JIqPUlUWUoFBikX+yTR+OcvLtAqWO11NPlNJw==",
                ),
            },
            example! {
                title: "Replace with SHA-256 hash using hex encoding",
                source: r#"redact("my id is 123456", filters: [r'\d+'], redactor: {"type": "sha2", "variant": "SHA-256", "encoding": "base16"})"#,
                result: Ok(
                    "my id is 8d969eef6ecad3c29a3a629280e686cf0c3f5d5a86aff3ca12020c923adc6c92",
                ),
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

#[allow(unpredictable_function_pointer_comparisons)]
/// The recipe for redacting the matched filters.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum Redactor {
    #[default]
    Full,
    /// Replace with a fixed string
    Text(String), // possible optimization: use Arc<str> instead of String to speed up cloning
    // using function pointers simplifies the code,
    // but the Debug implmentation probably isn't very useful
    // alternatively we could have a separate variant for each hash algorithm/variant combination
    // we could also create a custom Debug implementation that does a comparison of the fn pointer
    // to function pointers we might use.
    /// Replace with a hash of the redacted content
    Hash {
        encoder: Encoder,
        hasher: fn(Encoder, &[u8]) -> String,
    },
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
            Redactor::Hash { encoder, hasher } => {
                dst.push_str(&hasher(*encoder, original.as_bytes()));
            }
        }
    }

    fn from_object(obj: &ObjectMap) -> std::result::Result<Self, &'static str> {
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
                let hasher = if let Some(variant) = obj.get("variant") {
                    match variant
                        .as_bytes()
                        .ok_or("`variant` must be a string")?
                        .as_ref()
                    {
                        b"SHA-224" => encoded_hash::<sha_2::Sha224>,
                        b"SHA-256" => encoded_hash::<sha_2::Sha256>,
                        b"SHA-384" => encoded_hash::<sha_2::Sha384>,
                        b"SHA-512" => encoded_hash::<sha_2::Sha512>,
                        b"SHA-512/224" => encoded_hash::<sha_2::Sha512_224>,
                        b"SHA-512/256" => encoded_hash::<sha_2::Sha512_256>,
                        _ => return Err("invalid sha2 variant"),
                    }
                } else {
                    encoded_hash::<sha_2::Sha512_256>
                };
                let encoder = obj
                    .get("encoding")
                    .map(Encoder::try_from)
                    .transpose()?
                    .unwrap_or(Encoder::Base64);
                Ok(Redactor::Hash { hasher, encoder })
            }
            b"sha3" => {
                let hasher = if let Some(variant) = obj.get("variant") {
                    match variant
                        .as_bytes()
                        .ok_or("`variant must be a string")?
                        .as_ref()
                    {
                        b"SHA3-224" => encoded_hash::<sha_3::Sha3_224>,
                        b"SHA3-256" => encoded_hash::<sha_3::Sha3_256>,
                        b"SHA3-384" => encoded_hash::<sha_3::Sha3_384>,
                        b"SHA3-512" => encoded_hash::<sha_3::Sha3_512>,
                        _ => return Err("invalid sha2 variant"),
                    }
                } else {
                    encoded_hash::<sha_3::Sha3_512>
                };
                let encoder = obj
                    .get("encoding")
                    .map(Encoder::try_from)
                    .transpose()?
                    .unwrap_or(Encoder::Base64);
                Ok(Redactor::Hash { hasher, encoder })
            }
            _ => Err("unknown `type` for `redactor`"),
        }
    }
}

impl regex::Replacer for &Redactor {
    fn replace_append(&mut self, caps: &regex::Captures, dst: &mut String) {
        self.replace_str(&caps[0], dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        match self {
            Redactor::Full => Some(REDACTED.into()),
            Redactor::Text(s) => Some(s.into()),
            Redactor::Hash { .. } => None,
        }
    }
}

impl TryFrom<Value> for Redactor {
    type Error = &'static str;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Object(object) => Redactor::from_object(&object),
            Value::Bytes(bytes) => match bytes.as_ref() {
                b"full" => Ok(Redactor::Full),
                b"sha2" => Ok(Redactor::Hash {
                    hasher: encoded_hash::<sha_2::Sha512_256>,
                    encoder: Encoder::Base64,
                }),
                b"sha3" => Ok(Redactor::Hash {
                    hasher: encoded_hash::<sha_3::Sha3_512>,
                    encoder: Encoder::Base64,
                }),
                _ => Err("unknown name of redactor"),
            },
            _ => Err("unknown literal for redactor, must be redactor name or object"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Encoder {
    Base64,
    Base16,
}

impl TryFrom<&Value> for Encoder {
    type Error = &'static str;

    fn try_from(value: &Value) -> std::result::Result<Self, Self::Error> {
        match value.as_bytes().ok_or("encoding must be string")?.as_ref() {
            b"base64" => Ok(Self::Base64),
            b"base16" | b"hex" => Ok(Self::Base16),
            _ => Err("unexpected encoding"),
        }
    }
}

impl Encoder {
    fn encode(self, data: &[u8]) -> String {
        use Encoder::{Base16, Base64};
        match self {
            Base64 => base64_simd::STANDARD.encode_to_string(data),
            Base16 => base16::encode_lower(data),
        }
    }
}

/// Compute the hash of `data` using `T` as the digest, then encode it using `encoder`
/// to get a String
fn encoded_hash<T: digest::Digest>(encoder: Encoder, data: &[u8]) -> String {
    encoder.encode(&T::digest(data))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{btreemap, value};
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

        text_redactor {
            args: func_args![
                value: "my id is 123456",
                filters: vec![Regex::new(r"\d+").unwrap()],
                redactor: btreemap!{"type" => "text", "replacement" => "***"},
            ],
            want: Ok("my id is ***"),
            tdef: TypeDef::bytes().infallible(),
        }

        sha2 {
            args: func_args![
                value: "my id is 123456",
                filters: vec![Regex::new(r"\d+").unwrap()],
                redactor: "sha2",
            ],
            want: Ok("my id is GEtTedW1p6tC094dDKH+3B8P+xSnZz69AmpjaXRd63I="),
            tdef: TypeDef::bytes().infallible(),
        }

        sha3 {
            args: func_args![
                value: "my id is 123456",
                filters: vec![Regex::new(r"\d+").unwrap()],
                redactor: "sha3",
            ],
            want: Ok("my id is ZNCdmTDI7PeeUTFnpYjLdUObdizo+bIupZdl8yqnTKGdLx6X3JIqPUlUWUoFBikX+yTR+OcvLtAqWO11NPlNJw=="),
            tdef: TypeDef::bytes().infallible(),
        }

        sha256_hex {
            args: func_args![
                value: "my id is 123456",
                filters: vec![Regex::new(r"\d+").unwrap()],
                redactor: btreemap!{"type" => "sha2", "variant" => "SHA-256", "encoding" =>
                    "base16"},
            ],
            want: Ok("my id is 8d969eef6ecad3c29a3a629280e686cf0c3f5d5a86aff3ca12020c923adc6c92"),
            tdef: TypeDef::bytes().infallible(),
        }

        invalid_redactor {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec!["us_social_security_number"],
                 redactor: "not a redactor"
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }

        invalid_redactor_obj {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec!["us_social_security_number"],
                 redactor: btreemap!{"type" => "wrongtype"},
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }

        invalid_redactor_no_type {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec!["us_social_security_number"],
                 redactor: btreemap!{"key" => "value"},
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }

        invalid_hash_variant {
             args: func_args![
                 value: "hello 123456 world",
                 filters: vec!["us_social_security_number"],
                 redactor: btreemap!{"type" => "sha2", "variant" => "MD5"},
             ],
             want: Err("invalid argument"),
             tdef: TypeDef::bytes().infallible(),
        }
    ];
}
