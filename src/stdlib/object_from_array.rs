use super::util::ConstOrExpr;
use crate::compiler::prelude::*;

fn make_object(values: Vec<Value>) -> Resolved {
    values
        .into_iter()
        .map(make_key_value)
        .collect::<Result<_, _>>()
        .map(Value::Object)
}

fn make_key_value(value: Value) -> ExpressionResult<(KeyString, Value)> {
    value.try_array().map_err(Into::into).and_then(|array| {
        let mut iter = array.into_iter();
        let key: KeyString = match iter.next() {
            None => return Err("array value too short".into()),
            Some(Value::Bytes(key)) => String::from_utf8_lossy(&key).into(),
            Some(_) => return Err("object keys must be strings".into()),
        };
        let value = iter.next().unwrap_or(Value::Null);
        Ok((key, value))
    })
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectFromArray;

impl Function for ObjectFromArray {
    fn identifier(&self) -> &'static str {
        "object_from_array"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "values",
            kind: kind::ARRAY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "create an object from an array of keys/value pairs",
            source: r#"object_from_array([["a", 1], ["b"], ["c", true, 3, 4]])"#,
            result: Ok(r#"{"a": 1, "b": null, "c": true}"#),
        }]
    }

    fn compile(
        &self,
        state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let values = ConstOrExpr::new(arguments.required("values"), state);

        Ok(OFAFn { values }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct OFAFn {
    values: ConstOrExpr,
}

impl FunctionExpression for OFAFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        make_object(self.values.resolve(ctx)?.try_array()?)
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
    ];
}
