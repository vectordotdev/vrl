use xxhash_rust::{xxh3, xxh32, xxh64};

use crate::compiler::prelude::*;

#[allow(clippy::cast_possible_wrap)]
fn xxhash(value: Value, ctx: &mut Context) -> Resolved {
    let bytes = value.try_bytes()?;
    let result = xxh32::xxh32(&bytes, 0);
    Ok(Value::from(result as i64))
}

#[allow(clippy::cast_possible_wrap)]
fn xxhash_with_options(value: Value, algorithm: Option<Value>, ctx: &mut Context) -> Resolved {
    let bytes = value.try_bytes()?;

    let algorithm_str = algorithm
        .map(|v| v.try_bytes())
        .transpose()?
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_else(|| "xxh32".to_string());

    match algorithm_str.as_str() {
        "xxh32" => {
            let result = xxh32::xxh32(&bytes, 0);
            Ok(Value::from(result as i64))
        }
        "xxh64" => {
            let result = xxh64::xxh64(&bytes, 0);
            Ok(Value::from(result as i64))
        }
        "xxh3_64" => {
            let result = xxh3::xxh3_64(&bytes);
            Ok(Value::from(result as i64))
        }
        "xxh3_128" => {
            let result = xxh3::xxh3_128(&bytes);
            // Convert u128 to string representation since VRL doesn't have native u128 support
            Ok(Value::from(result.to_string()))
        }
        _ => Err("algorithm must be either 'xxh32', 'xxh64', 'xxh3_64', or 'xxh3_128'".into()),
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
                keyword: "algorithm",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "calculate xxhash hash (xxh32 default)",
                source: r#"xxhash("foobar")"#,
                result: Ok("2654435761"),
            },
            Example {
                title: "calculate xxhash hash (xxh64)",
                source: r#"xxhash("foobar", algorithm: "xxh64")"#,
                result: Ok("-7444071767201028348"),
            },
            Example {
                title: "calculate xxhash3_64 hash",
                source: r#"xxhash("foobar", algorithm: "xxh3_64")"#,
                result: Ok("-8166901779493161352"),
            },
            Example {
                title: "calculate xxhash3_128 hash",
                source: r#"xxhash("foobar", algorithm: "xxh3_128")"#,
                result: Ok("303003700981207993820949119788962816296"),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let algorithm = arguments.optional("algorithm");

        match algorithm {
            Some(algorithm_expr) => Ok(xxhash_with_options_fn::new(value, algorithm_expr).into()),
            None => Ok(xxhash_fn::new(value).into()),
        }
    }
}

#[derive(Debug, Clone)]
struct xxhash_fn {
    value: Box<dyn Expression>,
}

impl xxhash_fn {
    fn new(value: Box<dyn Expression>) -> Self {
        Self { value }
    }
}

impl FunctionExpression for xxhash_fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        xxhash(value, ctx)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        TypeDef::integer()
    }
}

#[derive(Debug, Clone)]
struct xxhash_with_options_fn {
    value: Box<dyn Expression>,
    algorithm: Box<dyn Expression>,
}

impl xxhash_with_options_fn {
    fn new(value: Box<dyn Expression>, algorithm: Box<dyn Expression>) -> Self {
        Self { value, algorithm }
    }
}

impl FunctionExpression for xxhash_with_options_fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let algorithm = Some(self.algorithm.resolve(ctx)?);
        xxhash_with_options(value, algorithm, ctx)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        // Return union of integer (32/64-bit variants) and string (128-bit)
        TypeDef::integer().or_string()
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
            want: Ok(value!(4138058784)),
            tdef: TypeDef::integer(),
        }

        hash_xxh32_explicit {
            args: func_args![value: "foo", algorithm: "xxh32"],
            want: Ok(value!(4138058784)),
            tdef: TypeDef::integer().or_string(),
        }

        hash_xxh64 {
            args: func_args![value: "foo", algorithm: "xxh64"],
            want: Ok(value!(-3728692692368066520)),
            tdef: TypeDef::integer().or_string(),
        }

        hash_xxh3_64 {
            args: func_args![value: "foo", algorithm: "xxh3_64"],
            want: Ok(value!(-4636154005964105165)),
            tdef: TypeDef::integer().or_string(),
        }

        hash_xxh3_128 {
            args: func_args![value: "foo", algorithm: "xxh3_128"],
            want: Ok(value!("84696207739838802046286913206618428373")),
            tdef: TypeDef::integer().or_string(),
        }

        invalid_algorithm {
            args: func_args![value: "foo", algorithm: "xxh16"],
            want: Err("algorithm must be either 'xxh32', 'xxh64', 'xxh3_64', or 'xxh3_128'"),
            tdef: TypeDef::integer().or_string(),
        }

        empty_string_xxh32 {
            args: func_args![value: ""],
            want: Ok(value!(46947589)),
            tdef: TypeDef::integer(),
        }

        empty_string_xxh64 {
            args: func_args![value: "", algorithm: "xxh64"],
            want: Ok(value!(-1205034819632174695)),
            tdef: TypeDef::integer().or_string(),
        }

        empty_string_xxh3_64 {
            args: func_args![value: "", algorithm: "xxh3_64"],
            want: Ok(value!(-4963153986929159138)),
            tdef: TypeDef::integer().or_string(),
        }

        empty_string_xxh3_128 {
            args: func_args![value: "", algorithm: "xxh3_128"],
            want: Ok(value!("99394879406065805935928060743628289650")),
            tdef: TypeDef::integer().or_string(),
        }

        unicode_string_xxh32 {
            args: func_args![value: "ñoño"],
            want: Ok(value!(1159352444)),
            tdef: TypeDef::integer(),
        }

        unicode_string_xxh64 {
            args: func_args![value: "ñoño", algorithm: "xxh64"],
            want: Ok(value!(3293578300618893924)),
            tdef: TypeDef::integer().or_string(),
        }

        unicode_string_xxh3_64 {
            args: func_args![value: "ñoño", algorithm: "xxh3_64"],
            want: Ok(value!(7265014866508651963)),
            tdef: TypeDef::integer().or_string(),
        }

        unicode_string_xxh3_128 {
            args: func_args![value: "ñoño", algorithm: "xxh3_128"],
            want: Ok(value!("232859983847772695459159509959473584851")),
            tdef: TypeDef::integer().or_string(),
        }

        binary_data_xxh32 {
            args: func_args![value: value!([0, 1, 2, 3, 255])],
            want: Ok(value!(1161967623)),
            tdef: TypeDef::integer(),
        }

        binary_data_xxh64 {
            args: func_args![value: value!([0, 1, 2, 3, 255]), algorithm: "xxh64"],
            want: Ok(value!(-6875981080804642536)),
            tdef: TypeDef::integer().or_string(),
        }

        binary_data_xxh3_64 {
            args: func_args![value: value!([0, 1, 2, 3, 255]), algorithm: "xxh3_64"],
            want: Ok(value!(-7905757983058936925)),
            tdef: TypeDef::integer().or_string(),
        }

        binary_data_xxh3_128 {
            args: func_args![value: value!([0, 1, 2, 3, 255]), algorithm: "xxh3_128"],
            want: Ok(value!("107988036830263503854644659829616537907")),
            tdef: TypeDef::integer().or_string(),
        }

        long_string_xxh32 {
            args: func_args![value: "vrl xxhash hash function"],
            want: Ok(value!(1951877985)),
            tdef: TypeDef::integer(),
        }

        long_string_xxh64 {
            args: func_args![value: "vrl xxhash hash function", algorithm: "xxh64"],
            want: Ok(value!(5103503319754928801)),
            tdef: TypeDef::integer().or_string(),
        }

        long_string_xxh3_64 {
            args: func_args![value: "vrl xxhash hash function", algorithm: "xxh3_64"],
            want: Ok(value!(-1476804297949976525)),
            tdef: TypeDef::integer().or_string(),
        }

        long_string_xxh3_128 {
            args: func_args![value: "vrl xxhash hash function", algorithm: "xxh3_128"],
            want: Ok(value!("229055459936831226905346831779542577667")),
            tdef: TypeDef::integer().or_string(),
        }

        large_input_xxh32 {
            args: func_args![value: "a".repeat(1024)],
            want: Ok(value!(1766643583)),
            tdef: TypeDef::integer(),
        }

        large_input_xxh64 {
            args: func_args![value: "a".repeat(1024), algorithm: "xxh64"],
            want: Ok(value!(5934086765239761821)),
            tdef: TypeDef::integer().or_string(),
        }

        large_input_xxh3_64 {
            args: func_args![value: "a".repeat(1024), algorithm: "xxh3_64"],
            want: Ok(value!(-4329874863901799357)),
            tdef: TypeDef::integer().or_string(),
        }

        large_input_xxh3_128 {
            args: func_args![value: "a".repeat(1024), algorithm: "xxh3_128"],
            want: Ok(value!("181467889456669024226987041395653829229")),
            tdef: TypeDef::integer().or_string(),
        }
    ];
}
