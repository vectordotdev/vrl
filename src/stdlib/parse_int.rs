use crate::compiler::prelude::*;

fn parse_int(value: &Value, base: Option<Value>) -> Resolved {
    let string = value.try_bytes_utf8_lossy()?;
    let (base, index) = match base {
        Some(base) => {
            let base = base.try_integer()?;

            if !(2..=36).contains(&base) {
                return Err(format!(
                    "invalid base {value}: must be be between 2 and 36 (inclusive)"
                )
                .into());
            }
            // TODO consider removal options
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            (base as u32, 0)
        }
        None => match string.chars().next() {
            Some('0') => match string.chars().nth(1) {
                Some('b') => (2, 2),
                Some('o') => (8, 2),
                Some('x') => (16, 2),
                _ => (8, 0),
            },
            Some(_) => (10u32, 0),
            None => return Err("value is empty".into()),
        },
    };
    let converted = i64::from_str_radix(&string[index..], base)
        .map_err(|err| format!("could not parse integer: {err}"))?;

    Ok(converted.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ParseInt;

impl Function for ParseInt {
    fn identifier(&self) -> &'static str {
        "parse_int"
    }

    fn usage(&self) -> &'static str {
        "Parses the string `value` representing a number in an optional base/radix to an integer."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
                description: "The string to parse.",
                default: None,
            },
            Parameter {
                keyword: "base",
                kind: kind::INTEGER,
                required: false,
                description: "The base the number is in. Must be between 2 and 36 (inclusive).

If unspecified, the string prefix is used to
determine the base: \"0b\", 8 for \"0\" or \"0o\", 16 for \"0x\",
and 10 otherwise.",
                default: None,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse decimal",
                source: r#"parse_int!("-42")"#,
                result: Ok("-42"),
            },
            example! {
                title: "Parse binary",
                source: r#"parse_int!("0b1001")"#,
                result: Ok("9"),
            },
            example! {
                title: "Parse octal",
                source: r#"parse_int!("0o42")"#,
                result: Ok("34"),
            },
            example! {
                title: "Parse hexadecimal",
                source: r#"parse_int!("0x2a")"#,
                result: Ok("42"),
            },
            example! {
                title: "Parse explicit base",
                source: r#"parse_int!("2a", 17)"#,
                result: Ok("44"),
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
        let base = arguments.optional("base");

        Ok(ParseIntFn { value, base }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseIntFn {
    value: Box<dyn Expression>,
    base: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let base = self
            .base
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        parse_int(&value, base)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        parse_int => ParseInt;

        decimal {
            args: func_args![value: "-42"],
            want: Ok(-42),
            tdef: TypeDef::integer().fallible(),
        }

        binary {
            args: func_args![value: "0b1001"],
            want: Ok(9),
            tdef: TypeDef::integer().fallible(),
        }

        octal {
            args: func_args![value: "042"],
            want: Ok(34),
            tdef: TypeDef::integer().fallible(),
        }

        hexadecimal {
            args: func_args![value: "0x2a"],
            want: Ok(42),
            tdef: TypeDef::integer().fallible(),
        }

        zero {
            args: func_args![value: "0"],
            want: Ok(0),
            tdef: TypeDef::integer().fallible(),
        }

        explicit_hexadecimal {
            args: func_args![value: "2a", base: 16],
            want: Ok(42),
            tdef: TypeDef::integer().fallible(),
        }
    ];
}
