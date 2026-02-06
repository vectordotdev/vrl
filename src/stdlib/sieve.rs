use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_REPLACE_SINGLE: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("")));
static DEFAULT_REPLACE_REPEATED: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The original string.",
            default: None,
        },
        Parameter {
            keyword: "permitted_characters",
            kind: kind::REGEX,
            required: true,
            description: "Keep all matches of this pattern.",
            default: None,
        },
        Parameter {
            keyword: "replace_single",
            kind: kind::BYTES,
            required: false,
            description: "The string to use to replace single rejected characters.",
            default: Some(&DEFAULT_REPLACE_SINGLE),
        },
        Parameter {
            keyword: "replace_repeated",
            kind: kind::BYTES,
            required: false,
            description: "The string to use to replace multiple sequential instances of rejected characters.",
            default: Some(&DEFAULT_REPLACE_REPEATED),
        },
    ]
});

fn sieve(
    value: &Value,
    permitted_characters: Value,
    replace_single: &Value,
    replace_repeated: &Value,
) -> Resolved {
    let value = value.try_bytes_utf8_lossy()?;
    let replace_single = replace_single.try_bytes_utf8_lossy()?;
    let replace_repeated = replace_repeated.try_bytes_utf8_lossy()?;

    match permitted_characters {
        Value::Regex(regex) => {
            let mut result = String::with_capacity(value.len());
            let mut last_end = 0;
            for m in regex.find_iter(&value) {
                match m.start() - last_end {
                    l if l > 1 => result += &replace_repeated,
                    1 => result += &replace_single,
                    _ => (),
                }
                last_end = m.end();
                result += m.as_str();
            }
            Ok(result.into())
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::regex(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sieve;

impl Function for Sieve {
    fn identifier(&self) -> &'static str {
        "sieve"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Keeps only matches of `pattern` in `value`.

            This can be used to define patterns that are allowed in the string and
            remove everything else.
        "}
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
                title: "Keep only lowercase letters",
                source: r#"sieve("vector.dev/lowerUPPER", permitted_characters: r'[a-z]')"#,
                result: Ok("vectordevlower"),
            },
            example! {
                title: "Sieve with regex",
                source: r#"sieve("test123%456.فوائد.net.", r'[a-z0-9.]')"#,
                result: Ok("test123456..net."),
            },
            example! {
                title: "Custom replacements",
                source: r#"sieve("test123%456.فوائد.net.", r'[a-z.0-9]', replace_single: "X", replace_repeated: "<REMOVED>")"#,
                result: Ok("test123X456.<REMOVED>.net."),
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
        let permitted_characters = arguments.required("permitted_characters");
        let replace_single = arguments.optional("replace_single");
        let replace_repeated = arguments.optional("replace_repeated");

        Ok(SieveFn {
            value,
            permitted_characters,
            replace_single,
            replace_repeated,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct SieveFn {
    value: Box<dyn Expression>,
    permitted_characters: Box<dyn Expression>,
    replace_single: Option<Box<dyn Expression>>,
    replace_repeated: Option<Box<dyn Expression>>,
}

impl FunctionExpression for SieveFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let permitted_characters = self.permitted_characters.resolve(ctx)?;
        let replace_single = self
            .replace_single
            .map_resolve_with_default(ctx, || DEFAULT_REPLACE_SINGLE.clone())?;
        let replace_repeated = self
            .replace_repeated
            .map_resolve_with_default(ctx, || DEFAULT_REPLACE_REPEATED.clone())?;

        sieve(
            &value,
            permitted_characters,
            &replace_single,
            &replace_repeated,
        )
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        sieve => Sieve;

        lowercase_letters_only {
            args: func_args![value: value!("vector.dev"), permitted_characters: regex::Regex::new("[a-z]").unwrap()],
            want: Ok(value!("vectordev")),
            tdef: TypeDef::bytes().infallible(),
        }

        alphanumeric_and_dots {
            args: func_args![value: value!("37ccx6a5uf52a7dv2hfxgpmltji09x6xkg0zv6yxsoi4kqs9atmjh7k50dcjb7z.فوائد.net."), permitted_characters: regex::Regex::new("[a-z.0-9]").unwrap()],
            want: Ok(value!("37ccx6a5uf52a7dv2hfxgpmltji09x6xkg0zv6yxsoi4kqs9atmjh7k50dcjb7z..net.")),
            tdef: TypeDef::bytes().infallible(),
        }

        all_options {
            args: func_args![value: value!("test123%456.فوائد.net."), permitted_characters: regex::Regex::new("[a-z.0-9]").unwrap(), replace_single: "X", replace_repeated: "<REMOVED>"],
            want: Ok(value!("test123X456.<REMOVED>.net.")),
            tdef: TypeDef::bytes().infallible(),
        }

        replace_repeated {
            args: func_args![value: value!("37ccx6a5uf52a7dv2hfxgpmltji09x6xkg0zv6yxsoi4kqs9atmjh7k50dcjb7z.فوائد.net."), permitted_characters: regex::Regex::new(r"[\.]").unwrap(), replace_repeated: "<REMOVED>"],
            want: Ok(value!("<REMOVED>.<REMOVED>.<REMOVED>.")),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
