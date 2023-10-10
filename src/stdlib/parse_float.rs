use std::str::FromStr;

use crate::compiler::prelude::*;

fn parse_float(value: Value) -> Resolved {
    let string = value.try_bytes_utf8_lossy()?;
    let float = f64::from_str(&string).map_err(|err| format!("could not parse float: {err}"))?;
    let converted = NotNan::new(float).map_err(|_| "NaN number not supported")?;

    Ok(converted.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ParseFloat;

impl Function for ParseFloat {
    fn identifier(&self) -> &'static str {
        "parse_float"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "integer",
                source: r#"parse_float!("-42")"#,
                result: Ok("-42.0"),
            },
            Example {
                title: "float",
                source: r#"parse_float!("42.38")"#,
                result: Ok("42.38"),
            },
            Example {
                title: "scientific notation",
                source: r#"parse_float!("2.5e3")"#,
                result: Ok("2500.0"),
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

        Ok(ParseIntFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseIntFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        parse_float(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::float().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        parse_float => ParseFloat;

        integer {
            args: func_args![value: "-42"],
            want: Ok(-42.0),
            tdef: TypeDef::float().fallible(),
        }

        float {
            args: func_args![value: "42.38"],
            want: Ok(42.38),
            tdef: TypeDef::float().fallible(),
        }

        scientific_1 {
            args: func_args![value: "2.5e3"],
            want: Ok(2500.0),
            tdef: TypeDef::float().fallible(),
        }

        scientific_2 {
            args: func_args![value: "8.543e-2"],
            want: Ok(0.08543),
            tdef: TypeDef::float().fallible(),
        }

        positive_zero {
            args: func_args![value: "+0"],
            want: Ok(0.0),
            tdef: TypeDef::float().fallible(),
        }

        negative_zero {
            args: func_args![value: "-0"],
            want: Ok(-0.0),
            tdef: TypeDef::float().fallible(),
        }

        positive_infinity {
            args: func_args![value: "inf"],
            want: Ok(f64::INFINITY),
            tdef: TypeDef::float().fallible(),
        }

        negative_infinity {
            args: func_args![value: "-inf"],
            want: Ok(f64::NEG_INFINITY),
            tdef: TypeDef::float().fallible(),
        }

        nan {
            args: func_args![value: "Nan"],
            want: Err("NaN number not supported".to_string()),
            tdef: TypeDef::float().fallible(),
        }

        min {
            args: func_args![value: "-1.7976931348623157e+308"],
            want: Ok(f64::MIN),
            tdef: TypeDef::float().fallible(),
        }

        max {
            args: func_args![value: "1.7976931348623157e+308"],
            want: Ok(f64::MAX),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
