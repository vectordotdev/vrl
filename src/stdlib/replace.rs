use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_COUNT: LazyLock<Value> = LazyLock::new(|| Value::Integer(-1));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The original string.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "pattern",
            kind: kind::BYTES | kind::REGEX,
            required: true,
            description: "Replace all matches of this pattern. Can be a static string or a regular expression.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "with",
            kind: kind::BYTES,
            required: true,
            description: "The string that the matches are replaced with.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "count",
            kind: kind::INTEGER,
            required: false,
            description: "The maximum number of replacements to perform. `-1` means replace all matches.",
            default: Some(&DEFAULT_COUNT),
            enum_variants: None,
        },
    ]
});

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // TODO consider removal options
fn replace(value: &Value, with_value: &Value, count: Value, pattern: Value) -> Resolved {
    let value = value.try_bytes_utf8_lossy()?;
    let with = with_value.try_bytes_utf8_lossy()?;
    let count = count.try_integer()?;
    match pattern {
        Value::Bytes(bytes) => {
            let pattern = String::from_utf8_lossy(&bytes);
            let replaced = match count {
                i if i > 0 => value.replacen(pattern.as_ref(), &with, i as usize),
                i if i < 0 => value.replace(pattern.as_ref(), &with),
                _ => value.into_owned(),
            };

            Ok(replaced.into())
        }
        Value::Regex(regex) => {
            let replaced = match count {
                i if i > 0 => Bytes::copy_from_slice(
                    regex.replacen(&value, i as usize, with.as_ref()).as_bytes(),
                )
                .into(),
                i if i < 0 => {
                    Bytes::copy_from_slice(regex.replace_all(&value, with.as_ref()).as_bytes())
                        .into()
                }
                _ => value.into(),
            };

            Ok(replaced)
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::regex() | Kind::bytes(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Replace;

impl Function for Replace {
    fn identifier(&self) -> &'static str {
        "replace"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Replaces all matching instances of `pattern` in `value`.

            The `pattern` argument accepts regular expression capture groups.

            **Note when using capture groups**:
            - You will need to escape the `$` by using `$$` to avoid Vector interpreting it as an
              [environment variable when loading configuration](/docs/reference/environment_variables/#escaping)
            - If you want a literal `$` in the replacement pattern, you will also need to escape this
              with `$$`. When combined with environment variable interpolation in config files this
              means you will need to use `$$$$` to have a literal `$` in the replacement pattern.
        "}
    }

    fn category(&self) -> &'static str {
        Category::String.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Replace literal text",
                source: r#"replace("Apples and Bananas", "and", "not")"#,
                result: Ok("Apples not Bananas"),
            },
            example! {
                title: "Replace using regular expression",
                source: r#"replace("Apples and Bananas", r'(?i)bananas', "Pineapples")"#,
                result: Ok("Apples and Pineapples"),
            },
            example! {
                title: "Replace first instance",
                source: r#"replace("Bananas and Bananas", "Bananas", "Pineapples", count: 1)"#,
                result: Ok("Pineapples and Bananas"),
            },
            example! {
                title: "Replace with capture groups",
                source: r#"replace("foo123bar", r'foo(?P<num>\d+)bar', "$num")"#,
                result: Ok(r#""123""#),
            },
            example! {
                title: "Replace all",
                source: r#"replace("foobar", "o", "i")"#,
                result: Ok("fiibar"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let pattern = arguments.required("pattern");
        let with = arguments.required("with");
        let count = arguments.optional("count");

        Ok(ReplaceFn {
            value,
            pattern,
            with,
            count,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct ReplaceFn {
    value: Box<dyn Expression>,
    pattern: Box<dyn Expression>,
    with: Box<dyn Expression>,
    count: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ReplaceFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let with_value = self.with.resolve(ctx)?;
        let count = self
            .count
            .map_resolve_with_default(ctx, || DEFAULT_COUNT.clone())?;
        let pattern = self.pattern.resolve(ctx)?;

        replace(&value, &with_value, count, pattern)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
#[allow(clippy::trivial_regex)]
mod test {
    use super::*;

    test_function![
        replace => Replace;

        replace_string1 {
             args: func_args![value: "I like apples and bananas",
                              pattern: "a",
                              with: "o"
             ],
             want: Ok("I like opples ond bononos"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_string2 {
             args: func_args![value: "I like apples and bananas",
                              pattern: "a",
                              with: "o",
                              count: -1
             ],
             want: Ok("I like opples ond bononos"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_string3 {
             args: func_args![value: "I like apples and bananas",
                              pattern: "a",
                              with: "o",
                              count: 0
             ],
             want: Ok("I like apples and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_string4 {
             args: func_args![value: "I like apples and bananas",
                              pattern: "a",
                              with: "o",
                              count: 1
             ],
             want: Ok("I like opples and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_string5 {
             args: func_args![value: "I like apples and bananas",
                              pattern: "a",
                              with: "o",
                              count: 2
             ],
             want: Ok("I like opples ond bananas"),
             tdef: TypeDef::bytes().infallible(),
         }


        replace_regex1 {
             args: func_args![value: "I like opples ond bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o"
             ],
             want: Ok("I like opples ond bononos"),
             tdef: TypeDef::bytes().infallible(),
         }


        replace_regex2 {
             args: func_args![value: "I like apples and bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o",
                              count: -1
             ],
             want: Ok("I like opples ond bononos"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_regex3 {
             args: func_args![value: "I like apples and bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o",
                              count: 0
             ],
             want: Ok("I like apples and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_regex4 {
             args: func_args![value: "I like apples and bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o",
                              count: 1
             ],
             want: Ok("I like opples and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_regex5 {
             args: func_args![value: "I like apples and bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o",
                              count: 2
             ],
             want: Ok("I like opples ond bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_other {
            args: func_args![value: "I like apples and bananas",
                             pattern: "apples",
                             with: "biscuits"
            ],
             want: Ok( "I like biscuits and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_other2 {
             args: func_args![value: "I like apples and bananas",
                              pattern: regex::Regex::new("a").unwrap(),
                              with: "o",
                              count: 1
             ],
             want: Ok("I like opples and bananas"),
             tdef: TypeDef::bytes().infallible(),
         }

        replace_other3 {
            args: func_args![value: "I like [apples] and bananas",
                             pattern: regex::Regex::new("\\[apples\\]").unwrap(),
                             with: "biscuits"
            ],
            want: Ok("I like biscuits and bananas"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
