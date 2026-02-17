use super::util::ConstOrExpr;
use crate::compiler::prelude::*;

fn make_object_1(values: Vec<Value>) -> Resolved {
    values
        .into_iter()
        .filter_map(|kv| make_key_value(kv).transpose())
        .collect::<Result<_, _>>()
        .map(Value::Object)
}

fn make_object_2(keys: Vec<Value>, values: Vec<Value>) -> Resolved {
    keys.into_iter()
        .zip(values)
        .filter_map(|(key, value)| {
            make_key_string(key)
                .transpose()
                .map(|key| key.map(|key| (key, value)))
        })
        .collect::<Result<_, _>>()
        .map(Value::Object)
}

fn make_key_value(value: Value) -> ExpressionResult<Option<(KeyString, Value)>> {
    let array = value.try_array()?;
    let mut iter = array.into_iter();
    let Some(key) = iter.next() else {
        return Err("array value too short".into());
    };
    Ok(make_key_string(key)?.map(|key| (key, iter.next().unwrap_or(Value::Null))))
}

fn make_key_string(key: Value) -> ExpressionResult<Option<KeyString>> {
    match key {
        Value::Bytes(key) => Ok(Some(String::from_utf8_lossy(&key).into())),
        Value::Null => Ok(None),
        _ => Err("object keys must be strings".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectFromArray;

impl Function for ObjectFromArray {
    fn identifier(&self) -> &'static str {
        "object_from_array"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Iterate over either one array of arrays or a pair of arrays and create an object out of all the key-value pairs contained in them.
            With one array of arrays, any entries with no value use `null` instead.
            Any keys that are `null` skip the  corresponding value.

            If a single parameter is given, it must contain an array of all the input arrays.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Object.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`values` and `keys` must be arrays.",
            "If `keys` is not present, `values` must contain only arrays.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "`object_from_array` is considered fallible in the following cases: if any of the parameters is not an array; if only the `value` parameter is present and it is not an array of arrays; or if any of the keys are not either a string or `null`.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required(
                "values",
                kind::ARRAY,
                "The first array of elements, or the array of input arrays if no other parameter is present.",
            ),
            Parameter::optional(
                "keys",
                kind::ARRAY,
                "The second array of elements. If not present, the first parameter must contain all the arrays.",
            ),
        ];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Create an object from one array",
                source: r#"object_from_array([["one", 1], [null, 2], ["two", 3]])"#,
                result: Ok(r#"{ "one": 1, "two": 3 }"#),
            },
            example! {
                title: "Create an object from separate key and value arrays",
                source: r#"object_from_array([1, 2, 3], keys: ["one", null, "two"])"#,
                result: Ok(r#"{ "one": 1, "two": 3 }"#),
            },
            example! {
                title: "Create an object from a separate arrays of keys and values",
                source: r#"object_from_array(values: [1, null, true], keys: ["a", "b", "c"])"#,
                result: Ok(r#"{"a": 1, "b": null, "c": true}"#),
            },
        ]
    }

    fn compile(
        &self,
        state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let values = ConstOrExpr::new(arguments.required("values"), state);
        let keys = arguments
            .optional("keys")
            .map(|keys| ConstOrExpr::new(keys, state));

        Ok(OFAFn { keys, values }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct OFAFn {
    keys: Option<ConstOrExpr>,
    values: ConstOrExpr,
}

impl FunctionExpression for OFAFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let values = self.values.resolve(ctx)?.try_array()?;
        match &self.keys {
            None => make_object_1(values),
            Some(keys) => make_object_2(keys.resolve(ctx)?.try_array()?, values),
        }
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::object(Collection::any())
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        object_from_array => ObjectFromArray;

        makes_object_simple {
            args: func_args![values: value!([["foo", 1], ["bar", 2]])],
            want: Ok(value!({"foo": 1, "bar": 2})),
            tdef: TypeDef::object(Collection::any()),
        }

        uses_keys_parameter {
            args: func_args![keys: value!(["foo", "bar"]), values: value!([1, 2])],
            want: Ok(value!({"foo": 1, "bar": 2})),
            tdef: TypeDef::object(Collection::any()),
        }

        handles_missing_values {
            args: func_args![values: value!([["foo", 1], ["bar"]])],
            want: Ok(value!({"foo": 1, "bar": null})),
            tdef: TypeDef::object(Collection::any()),
        }

        drops_extra_values {
            args: func_args![values: value!([["foo", 1, 2, 3, 4]])],
            want: Ok(value!({"foo": 1})),
            tdef: TypeDef::object(Collection::any()),
        }

        errors_on_missing_keys {
            args: func_args![values: value!([["foo", 1], []])],
            want: Err("array value too short"),
            tdef: TypeDef::object(Collection::any()),
        }

        skips_null_keys1 {
            args: func_args![values: value!([["foo", 1], [null, 2], ["bar", 3]])],
            want: Ok(value!({"foo": 1, "bar": 3})),
            tdef: TypeDef::object(Collection::any()),
        }

        skips_null_keys2 {
            args: func_args![values: value!([1, 2, 3]), keys: value!(["foo", null, "bar"])],
            want: Ok(value!({"foo": 1, "bar": 3})),
            tdef: TypeDef::object(Collection::any()),
        }
    ];
}
