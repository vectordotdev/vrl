use crate::compiler::prelude::*;
use std::collections::{BTreeMap, HashMap};

fn tally(value: Value) -> Resolved {
    let value = value.try_array()?;
    let mut map: HashMap<String, usize> = HashMap::new();
    for value in value {
        match value {
            Value::String(s) => *map.entry(s.to_string()).or_insert(0) += 1,
            Value::Bytes(bytes) => {
                *map.entry(String::from_utf8_lossy(&bytes).into_owned())
                    .or_insert(0) += 1;
            }
            value => {
                return Err(format!("all values must be strings, found: {value:?}").into());
            }
        }
    }
    let map: BTreeMap<_, _> = map
        .into_iter()
        .map(|(k, v)| (k.into(), Value::from(v)))
        .collect();
    Ok(map.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Tally;

impl Function for Tally {
    fn identifier(&self) -> &'static str {
        "tally"
    }

    fn usage(&self) -> &'static str {
        "Counts the occurrences of each string value in the provided array and returns an object with the counts."
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "tally",
            source: r#"tally(["foo", "bar", "foo", "baz"])"#,
            result: Ok(r#"{"foo": 2, "bar": 1, "baz": 1}"#),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(TallyFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ARRAY,
            "The array of strings to count occurrences for.",
        )
        .with_element_kind(kind::BYTES)];
        PARAMETERS
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TallyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for TallyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        tally(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(Collection::from_unknown(Kind::integer()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        tally => Tally;

        default {
            args: func_args![
                value: value!(["bar", "foo", "baz", "foo"]),
            ],
            want: Ok(value!({"bar": 1, "foo": 2, "baz": 1})),
            tdef: TypeDef::object(Collection::from_unknown(Kind::integer())),
        }

        // Runtime error still fires; compiler-level fallibility is driven by
        // `element_kind(kind::BYTES)` on the parameter, not by `type_def()`.
        non_string_values {
            args: func_args![
                value: value!(["foo", [1,2,3], "123abc", 1, true, [1,2,3], "foo", true, 1]),
            ],
            want: Err("all values must be strings, found: Array([Integer(1), Integer(2), Integer(3)])"),
            tdef: TypeDef::object(Collection::from_unknown(Kind::integer())),
        }
    ];
}
