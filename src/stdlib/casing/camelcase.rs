use crate::compiler::prelude::*;

use convert_case::Case;

fn camelcase(value: Value, orig_case: Option<Case>) -> Resolved {
    super::convert_case(value, Case::Camel, orig_case)
}

#[derive(Clone, Copy, Debug)]
pub struct Camelcase;

impl Function for Camelcase {
    fn identifier(&self) -> &'static str {
        "camelcase"
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

        Ok(CamelcaseFn {
            value,
            original_case,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "camelcase",
            source: r#"camelcase("input_string")"#,
            result: Ok("inputString"),
        }]
    }
}

#[derive(Debug, Clone)]
struct CamelcaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
}

impl FunctionExpression for CamelcaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let original_case = self.original_case;
        camelcase(value, original_case)
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
        camelcase => Camelcase;

        simple {
            args: func_args![value: value!("into_camel"), original_case: "snake_case"],
            want: Ok(value!("intoCamel")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("into_camel")],
            want: Ok(value!("intoCamel")),
            tdef: TypeDef::bytes(),
        }
    ];
}
