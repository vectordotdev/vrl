use crate::compiler::prelude::*;
use crate::value;
use xxhash_rust::{xxh3, xxh32, xxh64};

const VALID_VARIANTS: &[&str] = &["XXH32", "XXH64", "XXH3-64", "XXH3-128"];

#[allow(clippy::cast_possible_wrap)]
fn xxhash(value: Value, variant: &Value) -> Resolved {
    let bytes = value.try_bytes()?;
    let variant = variant.try_bytes_utf8_lossy()?.as_ref().to_uppercase();

    match variant.as_str() {
        "XXH32" => {
            let result = xxh32::xxh32(&bytes, 0);
            Ok(Value::from(i64::from(result)))
        }
        "XXH64" => {
            let result = xxh64::xxh64(&bytes, 0);
            Ok(Value::from(result as i64))
        }
        "XXH3-64" => {
            let result = xxh3::xxh3_64(&bytes);
            Ok(Value::from(result as i64))
        }
        "XXH3-128" => {
            let result = xxh3::xxh3_128(&bytes);
            // Convert u128 to string representation since VRL doesn't have native u128 support
            Ok(Value::from(result.to_string()))
        }
        _ => Err("Variant must be either 'XXH32', 'XXH64', 'XXH3-64', or 'XXH3-128'".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Xxhash;

impl Function for Xxhash {
    fn identifier(&self) -> &'static str {
        "xxhash"
    }

    fn summary(&self) -> &'static str {
        "calculate xxhash hash"
    }

    fn usage(&self) -> &'static str {
        "xxhash"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "variant",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "calculate xxhash hash (XXH32 default)",
                source: r#"xxhash("foo")"#,
                result: Ok("3792637401"),
            },
            Example {
                title: "calculate xxhash hash (XXH32)",
                source: r#"xxhash("foo", "XXH32")"#,
                result: Ok("3792637401"),
            },
            Example {
                title: "calculate xxhash hash (XXH64)",
                source: r#"xxhash("foo", "XXH64")"#,
                result: Ok("3728699739546630719"),
            },
            Example {
                title: "calculate XXH3-64 hash",
                source: r#"xxhash("foo", "XXH3-64")"#,
                result: Ok("-6093828362558603894"),
            },
            Example {
                title: "calculate XXH3-128 hash",
                source: r#"xxhash("foo", "XXH3-128")"#,
                result: Ok(r#""161745101148472925293886522910304009610""#),
            },
        ]
    }

    fn compile(
        &self,
        _: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let variant = arguments.optional("variant");

        Ok(XxhashFn { value, variant }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct XxhashFn {
    value: Box<dyn Expression>,
    variant: Option<Box<dyn Expression>>,
}

impl FunctionExpression for XxhashFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let variant = match &self.variant {
            Some(variant) => variant.resolve(ctx)?,
            _ => value!("XXH32"),
        };

        xxhash(value, &variant)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let variant = self.variant.as_ref();
        let valid_static_variant = variant.is_none()
            || variant
                .and_then(|variant| variant.resolve_constant(state))
                .and_then(|variant| variant.try_bytes_utf8_lossy().map(|s| s.to_string()).ok())
                .is_some_and(|variant| VALID_VARIANTS.contains(&variant.to_uppercase().as_str()));

        if valid_static_variant {
            TypeDef::bytes().infallible()
        } else {
            TypeDef::bytes().fallible()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        xxhash => Xxhash;

    hash_xxh32_default {
        args: func_args![value: "foo"],
        want: Ok(value!(3_792_637_401_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    hash_xxh32 {
        args: func_args![value: "foo", variant: "XXH32"],
        want: Ok(value!(3_792_637_401_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    hash_xxh64 {
        args: func_args![value: "foo", variant: "XXH64"],
        want: Ok(value!(3_728_699_739_546_630_719_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    hash_xxh3_64 {
        args: func_args![value: "foo", variant: "XXH3-64"],
        want: Ok(value!(-6_093_828_362_558_603_894_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    hash_xxh3_128 {
        args: func_args![value: "foo", variant: "XXH3-128"],
        want: Ok(value!("161745101148472925293886522910304009610")),
        tdef: TypeDef::bytes().infallible(),
    }

    long_string_xxh32 {
        args: func_args![value: "vrl xxhash hash function"],
        want: Ok(value!(919_261_294_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    long_string_xxh64 {
        args: func_args![value: "vrl xxhash hash function", variant: "XXH64"],
        want: Ok(value!(7_826_295_616_420_964_813_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    long_string_xxh3_64 {
        args: func_args![value: "vrl xxhash hash function", variant: "XXH3-64"],
        want: Ok(value!(-7_714_906_473_624_552_998_i64)),
        tdef: TypeDef::bytes().infallible(),
    }

    long_string_xxh3_128 {
        args: func_args![value: "vrl xxhash hash function", variant: "XXH3-128"],
        want: Ok(value!("89621485359950851650871997518391357172")),
        tdef: TypeDef::bytes().infallible(),
    }

    hash_invalid_variant {
        args: func_args![value: "foo", variant: "XXH16"],
        want: Err("Variant must be either 'XXH32', 'XXH64', 'XXH3-64', or 'XXH3-128'"),
        tdef: TypeDef::bytes().fallible(),
    }
    ];
}
