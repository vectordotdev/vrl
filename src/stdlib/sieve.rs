use crate::compiler::prelude::*;

fn sieve(
    value: Value,
    permitted_characters: Value,
    replace_single: Value,
    replace_repeated: Value,
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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "permitted_characters",
                kind: kind::REGEX,
                required: true,
            },
            Parameter {
                keyword: "replace_single",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "replace_repeated",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "sieve simple",
            source: r#"sieve("vector.dev", permitted_characters: r'[a-z]')"#,
            result: Ok("vectordev"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let permitted_characters = arguments.required("permitted_characters");
        let replace_single = arguments
            .optional("replace_single")
            .unwrap_or_else(|| expr!(""));
        let replace_repeated = arguments
            .optional("replace_repeated")
            .unwrap_or_else(|| expr!(""));

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
    replace_single: Box<dyn Expression>,
    replace_repeated: Box<dyn Expression>,
}

impl FunctionExpression for SieveFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let permitted_characters = self.permitted_characters.resolve(ctx)?;
        let replace_single = self.replace_single.resolve(ctx)?;
        let replace_repeated = self.replace_repeated.resolve(ctx)?;

        sieve(
            value,
            permitted_characters,
            replace_single,
            replace_repeated,
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
