use crate::compiler::prelude::*;
use crate::value;
use xxhash_rust::{xxh3, xxh32, xxh64};

#[allow(clippy::cast_possible_wrap)]
fn xxhash(value: Value, variant: &Bytes) -> Resolved {
    let bytes = value.try_bytes()?;

    match variant.as_ref() {
        b"XXH32" => {
            let result = xxh32::xxh32(&bytes, 0);
            Ok(Value::from(result as i64))
        }
        b"XXH64" => {
            let result = xxh64::xxh64(&bytes, 0);
            Ok(Value::from(result as i64))
        }
        b"XXH3-64" => {
            let result = xxh3::xxh3_64(&bytes);
            Ok(Value::from(result as i64))
        }
        b"XXH3-128" => {
            let result = xxh3::xxh3_128(&bytes);
            // Convert u128 to string representation since VRL doesn't have native u128 support
            Ok(Value::from(result.to_string()))
        }
        _ => unreachable!("variant must be either 'XXH32', 'XXH64', 'XXH3-64', or 'XXH3-128'"),
    }
}

fn variants() -> Vec<Value> {
    vec![
        value!("XXH32"),
        value!("XXH64"),
        value!("XXH3-64"),
        value!("XXH3-128"),
    ]
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
                source: r#"xxhash("foobar", "XXH32")"#,
                result: Ok("3792637401"),
            },
            Example {
                title: "calculate xxhash hash (XXH64)",
                source: r#"xxhash("foobar", "XXH64")"#,
                result: Ok("-3728699739546630719"),
            },
            Example {
                title: "calculate XXH3-64 hash",
                source: r#"xxhash("foo", "XXH3-64")"#,
                result: Ok("-6093828362558603894"),
            },
            Example {
                title: "calculate XXH3-128 hash",
                source: r#"xxhash("foo", "XXH3-128")"#,
                result: Ok("161745101148472925293886522910304009610"),
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
        let variant = arguments
            .optional_enum("variant", &variants(), state)?
            .unwrap_or_else(|| value!("XXH32"))
            .try_bytes()
            .expect("variant not bytes");

        Ok(XxhashFn { value, variant }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct XxhashFn {
    value: Box<dyn Expression>,
    variant: Bytes,
}

impl FunctionExpression for XxhashFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let variant = &self.variant;

        xxhash(value, variant)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible().or_integer()
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
            want: Ok(value!(3792637401_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        hash_xxh32 {
            args: func_args![value: "foo", variant: "XXH32"],
            want: Ok(value!(3792637401_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        hash_xxh64 {
            args: func_args![value: "foo", variant: "XXH64"],
            want: Ok(value!(3728699739546630719_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        hash_xxh3_64 {
            args: func_args![value: "foo", variant: "XXH3-64"],
            want: Ok(value!(-6093828362558603894_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        hash_xxh3_128 {
            args: func_args![value: "foo", variant: "XXH3-128"],
            want: Ok(value!("161745101148472925293886522910304009610")),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        long_string_xxh32 {
            args: func_args![value: "vrl xxhash hash function"],
            want: Ok(value!(919261294_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        long_string_xxh64 {
            args: func_args![value: "vrl xxhash hash function", variant: "XXH64"],
            want: Ok(value!(7826295616420964813_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        long_string_xxh3_64 {
            args: func_args![value: "vrl xxhash hash function", variant: "XXH3-64"],
            want: Ok(value!(-7714906473624552998_i64)),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }

        long_string_xxh3_128 {
            args: func_args![value: "vrl xxhash hash function", variant: "XXH3-128"],
            want: Ok(value!("89621485359950851650871997518391357172")),
            tdef: TypeDef::bytes().fallible().or_integer(),
        }
    ];
}
