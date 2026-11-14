use crate::compiler::expression::{Expr, Variant};
use crate::compiler::prelude::*;
use crate::path::OwnedValuePath;

fn get(value: &Value, value_path: Value) -> Resolved {
    let path = match value_path {
        Value::Array(array) => OwnedValuePath::try_from(array)?,
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any()),
            }
            .into());
        }
    };
    Ok(value.get(&path).cloned().unwrap_or(Value::Null))
}

#[derive(Clone, Copy, Debug)]
pub struct Get;

impl Function for Get {
    fn identifier(&self) -> &'static str {
        "get"
    }

    fn description(&self) -> &'static str {
        indoc! {"
            Dynamically get the value of a given path.

            If you know the path you want to look up, use
            static paths such as `.foo.bar[1]` to get the value of that
            path. However, if you do not know the path names,
            use the dynamic `get` function to get the requested value.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Path.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["The `path` segment must be a string or an integer."]
    }

    fn return_kind(&self) -> u16 {
        kind::ANY
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required(
                "value",
                kind::OBJECT | kind::ARRAY,
                "The object or array to query.",
            ),
            Parameter::required(
                "path",
                kind::ARRAY,
                "An array of path segments to look for the value.",
            ),
        ];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Single-segment top-level field",
                source: r#"get!(value: {"foo": "bar"}, path: ["foo"])"#,
                result: Ok(r#""bar""#),
            },
            example! {
                title: "Returns null for unknown field",
                source: r#"get!(value: {"foo": "bar"}, path: ["baz"])"#,
                result: Ok("null"),
            },
            example! {
                title: "Multi-segment nested field",
                source: r#"get!(value: {"foo": { "bar": true }}, path: ["foo", "bar"])"#,
                result: Ok("true"),
            },
            example! {
                title: "Array indexing",
                source: "get!(value: [92, 42], path: [0])",
                result: Ok("92"),
            },
            example! {
                title: "Array indexing (negative)",
                source: r#"get!(value: ["foo", "bar", "baz"], path: [-2])"#,
                result: Ok(r#""bar""#),
            },
            example! {
                title: "Nested indexing",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", "bar", 1])"#,
                result: Ok("42"),
            },
            example! {
                title: "External target",
                source: r#"get!(value: ., path: ["foo"])"#,
                input: r#"{ "foo": true }"#,
                result: Ok("true"),
            },
            example! {
                title: "Variable",
                source: indoc! {r#"
                    var = { "foo": true }
                    get!(value: var, path: ["foo"])
                "#},
                result: Ok("true"),
            },
            example! {
                title: "Missing index",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", "bar", 1, -1])"#,
                result: Ok("null"),
            },
            example! {
                title: "Invalid indexing",
                source: r#"get!(value: [42], path: ["foo"])"#,
                result: Ok("null"),
            },
            example! {
                title: "Invalid segment type",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", true])"#,
                result: Err(
                    r#"function call error for "get" at (0:62): path segment must be either string or integer, not boolean"#,
                ),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required_expr("value");
        let path = arguments.required("path");

        // A structurally literal value (e.g. an inline lookup table) is
        // resolved once at compile time, not re-resolved per event.
        match structural_constant(&value) {
            Some(constant) => Ok(GetConstantFn { constant, path }.as_expr()),
            None => Ok(GetFn {
                value: Box::new(value),
                path,
            }
            .as_expr()),
        }
    }
}

/// Resolve a structurally literal expression (literals, and object/array
/// trees of literals) to its value. Unlike `Expression::resolve_constant`,
/// no compile-time variable state is consulted: state-based folding is
/// unsound when an earlier argument mutates the variable at runtime (vectordotdev/vrl#1799).
fn structural_constant(expr: &Expr) -> Option<Value> {
    fn literal_expr(expr: &Expr) -> Option<Value> {
        match expr {
            Expr::Literal(literal) => Some(literal.to_value()),
            Expr::Container(container) => match &container.variant {
                Variant::Object(object) => object
                    .iter()
                    .map(|(key, expr)| literal_expr(expr).map(|v| (key.clone(), v)))
                    .collect::<Option<ObjectMap>>()
                    .map(Value::Object),
                Variant::Array(array) => array
                    .iter()
                    .map(literal_expr)
                    .collect::<Option<Vec<_>>>()
                    .map(Value::Array),
                // Grouped expressions take the generic path instead.
                Variant::Group(_) | Variant::Block(_) => None,
            },
            _ => None,
        }
    }

    literal_expr(expr)
}

#[derive(Debug, Clone)]
pub(crate) struct GetFn {
    value: Box<dyn Expression>,
    path: Box<dyn Expression>,
}

impl FunctionExpression for GetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let path = self.path.resolve(ctx)?;
        let value = self.value.resolve(ctx)?;

        get(&value, path)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::any().fallible()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GetConstantFn {
    constant: Value,
    path: Box<dyn Expression>,
}

impl FunctionExpression for GetConstantFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let path = self.path.resolve(ctx)?;

        get(&self.constant, path)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::any().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use std::collections::BTreeMap;

    test_function![
        get => Get;

        any {
            args: func_args![value: value!([42]), path: value!([0])],
            want: Ok(42),
            tdef: TypeDef::any().fallible(),
        }
    ];

    fn compile_get(value: Expr, path: Expr) -> Box<dyn Expression> {
        use crate::compiler::CompileConfig;

        let state = state::TypeState::default();
        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert("value", value);
        args.insert("path", path);
        Get.compile(&state, &mut ctx, args).unwrap()
    }

    fn resolve_with_target(expr: &dyn Expression, target: &mut Value) -> Resolved {
        let mut runtime_state = state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC);
        let mut ctx = Context::new(target, &mut runtime_state, &tz);
        expr.resolve(&mut ctx)
    }

    #[test]
    fn get_folds_literal_object() {
        use crate::compiler::expression::{Array, Container, Query, Target, container};
        use crate::path::{PathPrefix, parse_value_path};

        // path = [ .sev ] : array literal containing a dynamic query
        let dyn_path = || {
            let q = Query::new(
                Target::External(PathPrefix::Event),
                parse_value_path(".sev").unwrap(),
            );
            Expr::Container(Container::new(container::Variant::Array(Array::from(
                vec![Expr::Query(q)],
            ))))
        };

        let table = value!({"emerg": 21, "info": 9});
        let expr = compile_get(table.into(), dyn_path());

        let debug = format!("{expr:?}");
        assert!(debug.contains("GetConstantFn"), "expected fold: {debug}");

        let mut target = value!({"sev": "emerg"});
        assert_eq!(
            resolve_with_target(expr.as_ref(), &mut target).unwrap(),
            Value::from(21)
        );

        // dynamic path: same folded table, different key
        let expr = compile_get(value!({"emerg": 21, "info": 9}).into(), dyn_path());
        let mut target = value!({"sev": "info"});
        assert_eq!(
            resolve_with_target(expr.as_ref(), &mut target).unwrap(),
            Value::from(9)
        );

        // unknown key resolves to null
        let mut target = value!({"sev": "debug"});
        assert_eq!(
            resolve_with_target(expr.as_ref(), &mut target).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn get_folds_literal_array() {
        let expr = compile_get(
            value!([92, 42]).into(),
            Value::Array(vec![Value::from(-1_i64)]).into(),
        );
        let mut target = value!({});
        assert_eq!(
            resolve_with_target(expr.as_ref(), &mut target).unwrap(),
            Value::from(42)
        );
    }

    #[test]
    fn get_folded_result_is_independent() {
        // mutating a returned nested object must not affect subsequent gets
        let expr = compile_get(
            value!({"cfg": {"retries": 3}}).into(),
            value!(["cfg"]).into(),
        );

        let mut target = value!({});
        let mut v1 = resolve_with_target(expr.as_ref(), &mut target).unwrap();
        if let Value::Object(map) = &mut v1 {
            map.insert("retries".into(), value!(99));
        } else {
            panic!("expected object");
        }

        let v2 = resolve_with_target(expr.as_ref(), &mut target).unwrap();
        assert_eq!(v2, value!({"retries": 3}));
    }

    #[test]
    fn get_variable_value_stays_dynamic() {
        // A variable argument must NOT be folded, even if its compile-time
        // state carries a constant value: the variable can be mutated at
        // runtime between the assignment and the call (see vectordotdev/vrl#1799).
        use crate::compiler::CompileConfig;
        use crate::compiler::expression::Variable;
        use crate::compiler::parser::Ident;
        use crate::compiler::type_def::Details;

        let mut state = state::TypeState::default();
        state.local.insert_variable(
            Ident::new("foo"),
            Details {
                type_def: TypeDef::object(Collection::any()),
                value: Some(value!({"k": 1})),
            },
        );
        let var = Variable::new((0, 0).into(), Ident::new("foo"), &state.local).unwrap();

        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert("value", var.into());
        args.insert("path", value!(["k"]).into());

        let expr = Get.compile(&state, &mut ctx, args).unwrap();
        let debug = format!("{expr:?}");
        assert!(!debug.contains("GetConstantFn"), "unexpected fold: {debug}");
    }

    #[test]
    fn get_nested_variable_value_not_folded() {
        // vectordotdev/vrl#1799 regression: an object literal that references a variable
        // must NOT be folded, even though compile-time state knows the
        // variable's value. A side effect in an earlier-resolved argument
        // (e.g. the path) can mutate the variable at runtime before the
        // generic path would resolve the object.
        use crate::compiler::CompileConfig;
        use crate::compiler::expression::{Container, Object, Variable, Variant};
        use crate::compiler::parser::Ident;
        use crate::compiler::type_def::Details;
        use crate::value::KeyString;

        let mut state = state::TypeState::default();
        state.local.insert_variable(
            Ident::new("pat"),
            Details {
                type_def: TypeDef::bytes(),
                value: Some(value!("foo")),
            },
        );
        let var = Variable::new((0, 0).into(), Ident::new("pat"), &state.local).unwrap();

        let mut object = BTreeMap::new();
        object.insert(KeyString::from("k"), Expr::Variable(var));

        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert(
            "value",
            Expr::Container(Container::new(Variant::Object(Object::new(object)))),
        );
        args.insert("path", value!(["k"]).into());

        let expr = Get.compile(&state, &mut ctx, args).unwrap();
        let debug = format!("{expr:?}");
        assert!(!debug.contains("GetConstantFn"), "unexpected fold: {debug}");
    }

    #[test]
    fn get_generic_sees_runtime_variable_mutation() {
        // companion to the regression above: with the generic path chosen,
        // a runtime variable value (not the stale compile-time one) is used
        use crate::compiler::CompileConfig;
        use crate::compiler::expression::{Container, Object, Variable, Variant};
        use crate::compiler::parser::Ident;
        use crate::compiler::type_def::Details;
        use crate::value::KeyString;

        let mut state = state::TypeState::default();
        state.local.insert_variable(
            Ident::new("pat"),
            Details {
                type_def: TypeDef::bytes(),
                value: Some(value!("stale")),
            },
        );
        let var = Variable::new((0, 0).into(), Ident::new("pat"), &state.local).unwrap();

        let mut object = BTreeMap::new();
        object.insert(KeyString::from("k"), Expr::Variable(var));

        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert(
            "value",
            Expr::Container(Container::new(Variant::Object(Object::new(object)))),
        );
        args.insert("path", value!(["k"]).into());
        let expr = Get.compile(&state, &mut ctx, args).unwrap();

        // runtime state holds the CURRENT value of `pat`
        let mut target = value!({});
        let mut runtime_state = state::RuntimeState::default();
        runtime_state.insert_variable(Ident::new("pat"), value!("fresh"));
        let tz = crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC);
        let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);

        assert_eq!(expr.resolve(&mut ctx).unwrap(), value!("fresh"));
    }

    #[test]
    fn get_group_of_literal_stays_dynamic() {
        use crate::compiler::expression::{Container, Group, container};

        // ({"a": 1}) — parenthesized literals take the generic path
        // (correct results, just not folded)
        let inner = value!({"a": 1}).into();
        let group = Expr::Container(Container::new(container::Variant::Group(Group::new(inner))));
        let expr = compile_get(group, value!(["a"]).into());
        let debug = format!("{expr:?}");
        assert!(!debug.contains("GetConstantFn"), "unexpected fold: {debug}");

        let mut target = value!({});
        assert_eq!(
            resolve_with_target(expr.as_ref(), &mut target).unwrap(),
            Value::from(1)
        );
    }

    #[test]
    fn get_block_child_not_folded() {
        // a block inside the literal (statements/side effects) must not fold
        use crate::compiler::expression::{Block, Container, Object, Variant};
        use crate::value::KeyString;

        let mut object = BTreeMap::new();
        object.insert(
            KeyString::from("k"),
            Expr::Container(Container::new(Variant::Block(Block::new_scoped(vec![
                value!(1).into(),
            ])))),
        );

        let expr = compile_get(
            Expr::Container(Container::new(Variant::Object(Object::new(object)))),
            value!(["k"]).into(),
        );
        let debug = format!("{expr:?}");
        assert!(!debug.contains("GetConstantFn"), "unexpected fold: {debug}");
    }

    #[test]
    fn get_folded_matches_dynamic_parity() {
        // same table via literal (folded) and via external query (dynamic)
        // must produce identical results for a matrix of paths
        use crate::compiler::expression::{Query, Target};
        use crate::path::{PathPrefix, parse_value_path};

        let cases: Vec<(Value, Value)> = vec![
            (value!(["emerg"]), value!(21)),
            (value!(["info"]), value!(9)),
            (value!(["nope"]), Value::Null),
        ];

        for (path, want) in cases {
            let folded = compile_get(value!({"emerg": 21, "info": 9}).into(), path.clone().into());

            let query = Query::new(
                Target::External(PathPrefix::Event),
                parse_value_path(".tbl").unwrap(),
            );
            let dynamic = compile_get(query.into(), path.clone().into());
            let debug = format!("{dynamic:?}");
            assert!(debug.contains("GetFn"), "expected dynamic: {debug}");

            let mut target = value!({"tbl": {"emerg": 21, "info": 9}});
            assert_eq!(
                resolve_with_target(folded.as_ref(), &mut target).unwrap(),
                want
            );
            assert_eq!(
                resolve_with_target(dynamic.as_ref(), &mut target).unwrap(),
                want
            );
        }
    }
}

#[cfg(test)]
mod folded_edge_cases {
    use crate::compiler::expression::Expr;
    use crate::compiler::prelude::*;
    use crate::stdlib::Get;
    use crate::value;
    use crate::value::Value;

    fn compile_get(value: Expr, path: Expr) -> Box<dyn Expression> {
        use crate::compiler::CompileConfig;
        let state = state::TypeState::default();
        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert("value", value);
        args.insert("path", path);
        Get.compile(&state, &mut ctx, args).unwrap()
    }

    fn run(expr: &dyn Expression) -> Resolved {
        let mut target = value!({});
        let mut rs = state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC);
        let mut ctx = Context::new(&mut target, &mut rs, &tz);
        expr.resolve(&mut ctx)
    }

    #[test]
    fn get_folded_empty_tables_and_value_types() {
        // empty object/array tables fold and look up to null
        assert_eq!(
            run(compile_get(value!({}).into(), value!(["k"]).into()).as_ref()).unwrap(),
            Value::Null
        );
        assert_eq!(
            run(compile_get(value!([]).into(), value!([0]).into()).as_ref()).unwrap(),
            Value::Null
        );

        // all scalar value types round-trip through the folded table
        let expr = compile_get(
            value!({"s": "str", "i": 42, "f": 1.5, "b": true, "n": null, "nested": {"deep": [10, 20]}}).into(),
            Value::Array(vec![value!("nested"), value!("deep"), Value::from(-1_i64)]).into(),
        );
        assert_eq!(run(expr.as_ref()).unwrap(), Value::from(20));

        for (path, want) in [
            (value!(["s"]), value!("str")),
            (value!(["i"]), Value::from(42)),
            (value!(["f"]), Value::from(1.5)),
            (value!(["b"]), Value::from(true)),
            (value!(["n"]), Value::Null),
        ] {
            let expr = compile_get(
                value!({"s": "str", "i": 42, "f": 1.5, "b": true, "n": null}).into(),
                path.into(),
            );
            assert_eq!(run(expr.as_ref()).unwrap(), want);
        }
    }

    #[test]
    fn get_folded_error_and_lookup_parity() {
        // scalar literal value arg: kind system may pass it through; folded and
        // dynamic must behave the same (lookup on a scalar -> null)
        let folded = compile_get(Value::from(42).into(), value!([0]).into());
        let out = run(folded.as_ref()).unwrap();
        assert_eq!(out, Value::Null);

        // invalid path segment through the FOLDED path: same error as the
        // generic path produces (boolean segment)
        let folded = compile_get(value!({"a": 1}).into(), value!([true]).into());
        let err = run(folded.as_ref()).unwrap_err();
        assert!(err.to_string().contains("string or integer"), "got: {err}");

        // multi-segment into scalar leaf -> null (not an error)
        let folded = compile_get(value!({"a": 1}).into(), value!(["a", "b"]).into());
        assert_eq!(run(folded.as_ref()).unwrap(), Value::Null);

        // integer segment against object key -> null
        let folded = compile_get(value!({"a": 1}).into(), value!([0]).into());
        assert_eq!(run(folded.as_ref()).unwrap(), Value::Null);
    }
}
