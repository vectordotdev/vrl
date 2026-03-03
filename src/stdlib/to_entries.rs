use crate::compiler::prelude::*;
use crate::core::Value;
use crate::example;
use crate::prelude::{
    ArgumentList, Collection, Compiled, Example, Expression, FunctionCompileContext, kind,
};
use crate::value::ObjectMap;

#[derive(Clone, Debug, Copy)]
pub struct ToEntries;

fn build_entry(key: Value, value: Value) -> Value {
    let entry = ObjectMap::from([("key".into(), key), ("value".into(), value)]);
    Value::Object(entry)
}

fn to_entries(value: Value) -> Resolved {
    match value {
        Value::Object(object) => Ok(Value::Array(
            object
                .into_iter()
                .map(|(key, value)| build_entry(Value::from(key), value))
                .collect(),
        )),
        Value::Array(array) => {
            let entries = array
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    let key = i64::try_from(index)
                        .map_err(|_| ValueError::OutOfRange(Kind::integer()))?;
                    Ok(build_entry(Value::from(key), value))
                })
                .collect::<Result<Vec<_>, ValueError>>()?;
            Ok(Value::Array(entries))
        }
        other => Err(ValueError::Expected {
            got: other.kind(),
            expected: Kind::object(Collection::any()).or_array(Collection::any()),
        }
        .into()),
    }
}

impl Function for ToEntries {
    fn identifier(&self) -> &'static str {
        "to_entries"
    }

    fn usage(&self) -> &'static str {
        "Converts JSON objects or arrays into array of objects."
    }

    fn category(&self) -> &'static str {
        Category::Object.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &["The return array has the same length as the input collection."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The object or array to manipulate.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Manipulate empty object",
                source: "to_entries({})",
                result: Ok("[]"),
            },
            example! {
                title: "Manipulate object",
                source: r#"to_entries({ "foo": "bar"})"#,
                result: Ok(r#"[{ "key": "foo", "value": "bar" }]"#),
            },
            example! {
                title: "Manipulate array",
                source: "to_entries([1, 2])",
                result: Ok(r#"[{ "key": 0, "value": 1 }, { "key": 1, "value": 2 }]"#),
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
        Ok(ToEntriesFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct ToEntriesFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToEntriesFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        to_entries(value)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::array(Collection::any())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        to_entries => ToEntries;

        empty_object {
            args: func_args![value: value!({})],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::any()),
        }

        object {
            args: func_args![value: value!({foo: "bar"})],
            want: Ok(value!([{key: "foo", value: "bar"}])),
            tdef: TypeDef::array(Collection::any()),
        }

        array {
            args: func_args![value: value!([1, 2])],
            want: Ok(value!([{key: 0, value: 1}, {key: 1, value: 2}])),
            tdef: TypeDef::array(Collection::any()),
        }

        empty_array {
            args: func_args![value: value!([])],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::any()),
        }

        non_object {
            args: func_args![value: value!(true)],
            want: Err("expected array or object, got boolean"),
            tdef: TypeDef::array(Collection::any()),
        }
    ];
}
