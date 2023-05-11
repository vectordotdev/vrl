use crate::compiler::prelude::*;
use std::collections::{BTreeMap, HashMap};

fn tally(value: Value) -> Resolved {
    let value = value.try_array()?;
    #[allow(clippy::mutable_key_type)] // false positive due to bytes::Bytes
    let mut map: HashMap<Bytes, usize> = HashMap::new();
    for value in value {
        if let Value::Bytes(value) = value {
            *map.entry(value).or_insert(0) += 1;
        } else {
            return Err(format!("all values must be strings, found: {value:?}").into());
        }
    }
    let map: BTreeMap<_, _> = map
        .into_iter()
        .map(|(k, v)| (String::from_utf8_lossy(&k).into_owned(), Value::from(v)))
        .collect();
    Ok(map.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Tally;

impl Function for Tally {
    fn identifier(&self) -> &'static str {
        "tally"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "tally",
            source: r#"tally!(["foo", "bar", "foo", "baz"])"#,
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
        &[Parameter {
            keyword: "value",
            kind: kind::ARRAY,
            required: true,
        }]
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
        TypeDef::object(Collection::from_unknown(Kind::integer())).fallible()
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
            tdef: TypeDef::object(Collection::from_unknown(Kind::integer())).fallible(),
        }

        non_string_values {
            args: func_args![
                value: value!(["foo", [1,2,3], "123abc", 1, true, [1,2,3], "foo", true, 1]),
            ],
            want: Err("all values must be strings, found: Array([Integer(1), Integer(2), Integer(3)])"),
            tdef: TypeDef::object(Collection::from_unknown(Kind::integer())).fallible(),
        }
    ];
}
