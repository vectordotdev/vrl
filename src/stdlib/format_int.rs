use std::collections::VecDeque;

use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_BASE: LazyLock<Value> = LazyLock::new(|| Value::Integer(10));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::INTEGER,
            required: true,
            description: "The number to format.",
            default: None,
        },
        Parameter {
            keyword: "base",
            kind: kind::INTEGER,
            required: false,
            description: "The base to format the number in. Must be between 2 and 36 (inclusive).",
            default: Some(&DEFAULT_BASE),
        },
    ]
});

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // TODO consider removal options
fn format_int(value: Value, base: Value) -> Resolved {
    let value = value.try_integer()?;
    let base = base.try_integer()?;
    if !(2..=36).contains(&base) {
        return Err(format!("invalid base {base}: must be be between 2 and 36 (inclusive)").into());
    }

    let converted = format_radix(value, base as u32);
    Ok(converted.into())
}

#[derive(Clone, Copy, Debug)]
pub struct FormatInt;

impl Function for FormatInt {
    fn identifier(&self) -> &'static str {
        "format_int"
    }

    fn usage(&self) -> &'static str {
        "Formats the integer `value` into a string representation using the given base/radix."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["The base is not between 2 and 36."]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let base = arguments.optional("base");

        Ok(FormatIntFn { value, base }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Format as a hexadecimal integer",
                source: "format_int!(42, 16)",
                result: Ok("2a"),
            },
            example! {
                title: "Format as a negative hexadecimal integer",
                source: "format_int!(-42, 16)",
                result: Ok("-2a"),
            },
            example! {
                title: "Format as a decimal integer (default base)",
                source: "format_int!(42)",
                // extra "s are needed to avoid being read as an integer by tests
                result: Ok("\"42\""),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct FormatIntFn {
    value: Box<dyn Expression>,
    base: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FormatIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let base = self
            .base
            .map_resolve_with_default(ctx, || DEFAULT_BASE.clone())?;

        format_int(value, base)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

// Formats x in the provided radix
//
// Panics if radix is < 2 or > 36
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // TODO consider removal options
fn format_radix(x: i64, radix: u32) -> String {
    let mut result: VecDeque<char> = VecDeque::new();

    let (mut x, negative) = if x < 0 {
        (-x as u64, true)
    } else {
        (x as u64, false)
    };

    loop {
        let m = (x % u64::from(radix)) as u32; // max of 35
        x /= u64::from(radix);

        result.push_front(std::char::from_digit(m, radix).unwrap());
        if x == 0 {
            break;
        }
    }

    if negative {
        result.push_front('-');
    }

    result.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    test_function![
        format_int => FormatInt;

        decimal {
            args: func_args![value: 42],
            want: Ok(value!("42")),
            tdef: TypeDef::bytes().fallible(),
        }

        hexidecimal {
            args: func_args![value: 42, base: 16],
            want: Ok(value!("2a")),
            tdef: TypeDef::bytes().fallible(),
        }

        negative_hexidecimal {
            args: func_args![value: -42, base: 16],
            want: Ok(value!("-2a")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
