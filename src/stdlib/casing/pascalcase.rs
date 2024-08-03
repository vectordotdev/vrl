use crate::compiler::prelude::*;

use convert_case::Case;

fn pascalcase(value: Value, orig_case: Option<Case>) -> Resolved {
    super::convert_case(value, Case::Pascal, orig_case)
}

#[derive(Clone, Copy, Debug)]
pub struct Pascalcase;

impl Function for Pascalcase {
    fn identifier(&self) -> &'static str {
        "pascalcase"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "original_case",
                kind: kind::BYTES,
                required: false,
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
        let original_case = arguments
            .optional_enum("original_case", &super::variants(), state)?
            .map(|b| {
                b.try_bytes_utf8_lossy()
                    .expect("cant convert to string")
                    .into_owned()
            })
            .map(super::into_case)
            .transpose()?;

        Ok(PascalcaseFn {
            value,
            original_case,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "pascalcase",
            source: r#"pascalcase("input_string")"#,
            result: Ok("InputString"),
        }]
    }
}

#[derive(Debug, Clone)]
struct PascalcaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
}

impl FunctionExpression for PascalcaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let original_case = self.original_case;
        pascalcase(value, original_case)
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
        pascalcase => Pascalcase;

        simple {
            args: func_args![value: value!("input_string"), original_case: "snake_case"],
            want: Ok(value!("InputString")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("input_string")],
            want: Ok(value!("InputString")),
            tdef: TypeDef::bytes(),
        }
    ];
}
