use crate::compiler::prelude::*;

fn map_keys<T>(
    value: Value,
    recursive: bool,
    ctx: &mut Context,
    runner: closure::Runner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Resolved,
{
    let mut iter = value.into_iter(recursive);

    for item in iter.by_ref() {
        if let IterItem::KeyValue(key, _) = item {
            runner.map_key(ctx, key)?;
        }
    }

    Ok(iter.into())
}

#[derive(Clone, Copy, Debug)]
pub struct MapKeys;

impl Function for MapKeys {
    fn identifier(&self) -> &'static str {
        "map_keys"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT,
                required: true,
            },
            Parameter {
                keyword: "recursive",
                kind: kind::BOOLEAN,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "map object keys",
                source: r#"map_keys({ "a": 1, "b": 2 }) -> |key| { upcase(key) }"#,
                result: Ok(r#"{ "A": 1, "B": 2 }"#),
            },
            Example {
                title: "recursively map object keys",
                source: r#"map_keys({ "a": 1, "b": [{ "c": 2 }, { "d": 3 }], "e": { "f": 4 } }, recursive: true) -> |key| { upcase(key) }"#,
                result: Ok(r#"{ "A": 1, "B": [{ "C": 2 }, { "D": 3 }], "E": { "F": 4 } }"#),
            },
            Example {
                title: "map nested object keys",
                source: r#"map_keys({ "a": 1, "b": { "c": 2, "d": 3, "e": { "f": 4 } } }.b) -> |key| { upcase(key) }"#,
                result: Ok(r#"{ "C": 2, "D": 3, "E": { "f": 4 } }"#),
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
        let recursive = arguments.optional("recursive");
        let closure = arguments.required_closure()?;

        Ok(MapKeysFn {
            value,
            recursive,
            closure,
        }
        .as_expr())
    }

    fn closure(&self) -> Option<closure::Definition> {
        use closure::{Definition, Input, Output, Variable, VariableKind};

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::object(Collection::any()),
                variables: vec![Variable {
                    kind: VariableKind::Exact(Kind::bytes()),
                }],
                output: Output::Kind(Kind::bytes()),
                example: Example {
                    title: "map object keys",
                    source: r#"map_keys({ "one" : 1, "two": 2 }) -> |key| { upcase(key) }"#,
                    result: Ok(r#"{ "ONE": 1, "TWO": 2 }"#),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct MapKeysFn {
    value: Box<dyn Expression>,
    recursive: Option<Box<dyn Expression>>,
    closure: FunctionClosure,
}

impl FunctionExpression for MapKeysFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let recursive = match &self.recursive {
            None => false,
            Some(expr) => expr.resolve(ctx)?.try_boolean()?,
        };

        let value = self.value.resolve(ctx)?;
        let FunctionClosure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        map_keys(value, recursive, ctx, runner)
    }

    fn type_def(&self, ctx: &state::TypeState) -> TypeDef {
        self.value.type_def(ctx)
    }
}
