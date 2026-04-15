use crate::compiler::prelude::*;
use crate::prelude::{
    ArgumentList, Collection, Compiled, Example, Expression, FunctionCompileContext, kind,
};
use crate::value::{KeyString, ObjectMap};

fn make_key_string(key: Value) -> ExpressionResult<KeyString> {
    match key {
        Value::Bytes(key) => Ok(String::from_utf8_lossy(&key).into()),
        _ => Err("object keys must be strings".into()),
    }
}

fn select_key(entry: &ObjectMap) -> Value {
    ["key", "Key", "name", "Name"]
        .into_iter()
        .filter_map(|alias| entry.get(alias).cloned())
        .find(|key| !matches!(key, Value::Null | Value::Boolean(false)))
        .unwrap_or(Value::Null)
}

fn from_entries(value: Value) -> Resolved {
    let array = value.try_array()?;
    let mut object = ObjectMap::new();

    for entry in array {
        let mut entry = entry.try_object()?;
        let key = select_key(&entry);
        let key = make_key_string(key)?;
        let value = entry
            .remove("value")
            .or_else(|| entry.remove("Value"))
            .unwrap_or(Value::Null);
        object.insert(key, value);
    }

    Ok(Value::Object(object))
}

#[derive(Clone, Debug, Copy)]
pub struct FromEntries;

impl Function for FromEntries {
    fn identifier(&self) -> &'static str {
        "from_entries"
    }

    fn usage(&self) -> &'static str {
        "Converts array of key/value objects into an object."
    }

    fn category(&self) -> &'static str {
        Category::Object.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &["Returns an object composed from the array entries."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ARRAY,
            required: true,
            description: "The array of key/value objects to convert.",
            default: None,
            enum_variants: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Manipulate empty array",
                source: "from_entries([])",
                result: Ok("{}"),
            },
            example! {
                title: "Manipulate array",
                source: r#"from_entries([{ "key": "foo", "value": "bar" }])"#,
                result: Ok(r#"{ "foo": "bar" }"#),
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
        Ok(FromEntriesFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct FromEntriesFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for FromEntriesFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        from_entries(value)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::object(Collection::any())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        from_entries => FromEntries;

        empty_array {
            args: func_args![value: value!([])],
            want: Ok(value!({})),
            tdef: TypeDef::object(Collection::any()),
        }

        array {
            args: func_args![value: value!([{key: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        array_with_capitalized_aliases {
            args: func_args![value: value!([{Key: "foo", Value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        array_with_mixed_aliases {
            args: func_args![value: value!([{Key: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        array_with_name_aliases {
            args: func_args![value: value!([{name: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        array_with_capitalized_name_aliases {
            args: func_args![value: value!([{Name: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        key_falls_back_when_null {
            args: func_args![value: value!([{key: null, Key: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        key_falls_back_when_false {
            args: func_args![value: value!([{key: false, Key: "foo", value: "bar"}])],
            want: Ok(value!({foo: "bar"})),
            tdef: TypeDef::object(Collection::any()),
        }

        missing_value_defaults_to_null {
            args: func_args![value: value!([{key: "foo"}])],
            want: Ok(value!({foo: null})),
            tdef: TypeDef::object(Collection::any()),
        }

        non_array {
            args: func_args![value: value!(true)],
            want: Err("expected array, got boolean"),
            tdef: TypeDef::object(Collection::any()),
        }

        entry_not_object {
            args: func_args![value: value!([true])],
            want: Err("expected object, got boolean"),
            tdef: TypeDef::object(Collection::any()),
        }

        key_not_string {
            args: func_args![value: value!([{key: 1, value: "bar"}])],
            want: Err("object keys must be strings"),
            tdef: TypeDef::object(Collection::any()),
        }
    ];
}
