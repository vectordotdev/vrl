use crate::compiler::expression::{Expr, Target};
use crate::compiler::parser::ast::Ident;
use crate::compiler::prelude::*;
use crate::path::{BorrowedSegment, OwnedSegment, OwnedTargetPath, OwnedValuePath, ValuePath};

fn validate_path_segments(segments: &[Value]) -> ExpressionResult<()> {
    for value in segments {
        match value {
            Value::String(_) | Value::Bytes(_) | Value::Integer(_) => {}
            _ => {
                return Err(format!(
                    "path segment must be either string or integer, not {}",
                    value.kind()
                )
                .into());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct BorrowedValuePath<'a>(&'a [Value]);

#[derive(Clone)]
struct BorrowedValuePathIter<'a>(std::slice::Iter<'a, Value>);

impl<'a> ValuePath<'a> for BorrowedValuePath<'a> {
    type Iter = BorrowedValuePathIter<'a>;

    fn segment_iter(&self) -> Self::Iter {
        BorrowedValuePathIter(self.0.iter())
    }
}

impl<'a> Iterator for BorrowedValuePathIter<'a> {
    type Item = BorrowedSegment<'a>;

    #[allow(clippy::cast_possible_truncation)]
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next()? {
            Value::Integer(i) => Some(BorrowedSegment::Index(*i as isize)),
            other => other.to_str_lossy().map(BorrowedSegment::Field),
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn try_compile_const_path(value: &Value) -> Option<OwnedValuePath> {
    let Value::Array(segments) = value else {
        return None;
    };

    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        match segment {
            Value::Integer(i) => path.push(OwnedSegment::index(*i as isize)),
            other => {
                let field = other.to_str_lossy()?;
                path.push(OwnedSegment::field(field.as_ref()));
            }
        }
    }

    Some(OwnedValuePath { segments: path })
}

#[derive(Debug, Clone)]
enum ValueSource {
    External(OwnedTargetPath),
    Variable {
        ident: Ident,
        path: OwnedValuePath,
    },
    OwnedQuery {
        expr: Box<dyn Expression>,
        path: OwnedValuePath,
    },
    Owned(Box<dyn Expression>),
}

impl ValueSource {
    fn from_expr(expr: Expr) -> Self {
        match expr {
            Expr::Query(query) => match query.target() {
                Target::External(prefix) => Self::External(OwnedTargetPath {
                    prefix: *prefix,
                    path: query.path().clone(),
                }),
                Target::Internal(var) => Self::Variable {
                    ident: var.ident().clone(),
                    path: query.path().clone(),
                },
                Target::Container(container) => Self::OwnedQuery {
                    expr: Box::new(container.clone()),
                    path: query.path().clone(),
                },
                Target::FunctionCall(call) => Self::OwnedQuery {
                    expr: Box::new(call.clone()),
                    path: query.path().clone(),
                },
            },
            Expr::Variable(var) => Self::Variable {
                ident: var.ident().clone(),
                path: OwnedValuePath::root(),
            },
            other => Self::Owned(Box::new(other)),
        }
    }
}

fn extract_validated_segments(path_value: Value) -> ExpressionResult<Vec<Value>> {
    match path_value {
        Value::Array(segments) => {
            validate_path_segments(&segments)?;
            Ok(segments)
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::array(Collection::any()),
        }
        .into()),
    }
}

fn resolve_with_path<'a>(
    value: &ValueSource,
    ctx: &mut Context,
    path: impl ValuePath<'a>,
) -> Resolved {
    match value {
        ValueSource::External(target_path) => Ok(ctx
            .target()
            .target_get(target_path)
            .ok()
            .flatten()
            .and_then(|v| v.get(path))
            .cloned()
            .unwrap_or(Value::Null)),
        ValueSource::Variable {
            ident,
            path: var_path,
        } => Ok(ctx
            .state()
            .variable(ident)
            .and_then(|v| v.get(var_path))
            .and_then(|v| v.get(path))
            .cloned()
            .unwrap_or(Value::Null)),
        ValueSource::OwnedQuery {
            expr,
            path: query_path,
        } => {
            let mut v = expr.resolve(ctx)?;
            Ok(v.get_mut(query_path)
                .and_then(|sub| sub.get_mut(path))
                .map_or(Value::Null, |leaf| std::mem::replace(leaf, Value::Null)))
        }
        ValueSource::Owned(expr) => {
            let mut v = expr.resolve(ctx)?;
            Ok(v.get_mut(path)
                .map_or(Value::Null, |leaf| std::mem::replace(leaf, Value::Null)))
        }
    }
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
        Ok(GetFn::compile(&arguments).as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GetFn {
    value: ValueSource,
    path: ConstOrExpr<OwnedValuePath>,
}

impl GetFn {
    fn compile(arguments: &ArgumentList) -> Self {
        let value = ValueSource::from_expr(arguments.required_expr("value"));
        let path_expr = arguments.required("path");
        // Resolve against an empty TypeState so only state-independent constant
        // expressions are folded; captured local variables may be reassigned
        // across iterator closure invocations (e.g. `for_each`).
        let path = match path_expr
            .resolve_constant(&state::TypeState::default())
            .as_ref()
            .and_then(try_compile_const_path)
        {
            Some(compiled) => ConstOrExpr::Const(compiled),
            None => ConstOrExpr::Expr(path_expr),
        };

        Self { value, path }
    }
}

impl FunctionExpression for GetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        match &self.path {
            ConstOrExpr::Const(const_path) => resolve_with_path(&self.value, ctx, const_path),
            ConstOrExpr::Expr(path_expr) => {
                let path_value = path_expr.resolve(ctx)?;
                match &self.value {
                    ValueSource::External(target_path) => {
                        let segments = extract_validated_segments(path_value)?;
                        Ok(ctx
                            .target()
                            .target_get(target_path)
                            .ok()
                            .flatten()
                            .and_then(|v| v.get(BorrowedValuePath(&segments)))
                            .cloned()
                            .unwrap_or(Value::Null))
                    }
                    ValueSource::Variable {
                        ident,
                        path: var_path,
                    } => {
                        let segments = extract_validated_segments(path_value)?;
                        Ok(ctx
                            .state()
                            .variable(ident)
                            .and_then(|v| v.get(var_path))
                            .and_then(|v| v.get(BorrowedValuePath(&segments)))
                            .cloned()
                            .unwrap_or(Value::Null))
                    }
                    ValueSource::OwnedQuery {
                        expr,
                        path: query_path,
                    } => {
                        let mut owned = expr.resolve(ctx)?;
                        let segments = extract_validated_segments(path_value)?;
                        Ok(owned
                            .get_mut(query_path)
                            .and_then(|sub| sub.get_mut(BorrowedValuePath(&segments)))
                            .map_or(Value::Null, |leaf| std::mem::replace(leaf, Value::Null)))
                    }
                    ValueSource::Owned(expr) => {
                        let mut owned = expr.resolve(ctx)?;
                        let segments = extract_validated_segments(path_value)?;
                        Ok(owned
                            .get_mut(BorrowedValuePath(&segments))
                            .map_or(Value::Null, |leaf| std::mem::replace(leaf, Value::Null)))
                    }
                }
            }
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::any().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        get => Get;

        any {
            args: func_args![value: value!([42]), path: value!([0])],
            want: Ok(42),
            tdef: TypeDef::any().fallible(),
        }
    ];

    #[test]
    fn validate_path_segments_accepts_bytes_and_integers_and_rejects_others() {
        let valid = vec![
            Value::Bytes(bytes::Bytes::from_static(b"foo")),
            Value::Integer(-1),
            Value::Bytes(bytes::Bytes::from_static(b"\xff")),
        ];
        assert!(validate_path_segments(&valid).is_ok());
        assert!(validate_path_segments(&[]).is_ok());

        let invalid = vec![
            Value::Bytes(bytes::Bytes::from_static(b"missing")),
            Value::Boolean(true),
        ];
        let err = validate_path_segments(&invalid).unwrap_err();
        assert_eq!(
            err.to_string(),
            "path segment must be either string or integer, not boolean"
        );
    }

    #[test]
    fn borrowed_value_path_traverses_objects_arrays_non_utf8_and_bounds() {
        let target = value!({
            "foo": ["a", "b", "c"],
            "\u{FFFD}": 99,
        });

        let segs = vec![value!("foo"), value!(-2)];
        assert_eq!(target.get(BorrowedValuePath(&segs)), Some(&value!("b")));

        let oob = vec![value!("foo"), value!(-100)];
        assert_eq!(target.get(BorrowedValuePath(&oob)), None);

        let non_utf8 = vec![Value::Bytes(bytes::Bytes::from_static(b"\xff"))];
        assert_eq!(target.get(BorrowedValuePath(&non_utf8)), Some(&value!(99)));

        assert_eq!(target.get(BorrowedValuePath(&[])), Some(&target));
    }

    #[test]
    fn try_compile_const_path_handles_valid_and_invalid_constants() {
        let valid = Value::Array(vec![value!("nested"), value!(0), value!(-1)]);
        let compiled = try_compile_const_path(&valid).expect("valid constant path");
        assert_eq!(
            compiled.segments,
            vec![
                OwnedSegment::field("nested"),
                OwnedSegment::index(0),
                OwnedSegment::index(-1),
            ]
        );

        let non_utf8 = Value::Array(vec![Value::Bytes(bytes::Bytes::from_static(b"\xff"))]);
        let compiled_non_utf8 = try_compile_const_path(&non_utf8).expect("lossy utf8 path");
        assert_eq!(
            compiled_non_utf8.segments,
            vec![OwnedSegment::field("\u{FFFD}")]
        );

        assert!(try_compile_const_path(&value!(["foo", true])).is_none());
        assert!(try_compile_const_path(&value!("not_an_array")).is_none());
    }

    fn run_vrl(source: &str, target: Value, metadata: Value) -> Result<Value, String> {
        let fns = crate::stdlib::all();
        let compiled = crate::compiler::compile(source, &fns)
            .map_err(|diagnostics| format!("compile error: {diagnostics:?}"))?;
        let mut target = crate::compiler::TargetValue {
            value: target,
            metadata,
            secrets: crate::value::Secrets::default(),
        };
        let mut state = crate::compiler::state::RuntimeState::default();
        let timezone = crate::compiler::TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &timezone);
        compiled
            .program
            .resolve(&mut ctx)
            .map_err(|err| err.to_string())
    }

    fn compile_expr(source: &str) -> crate::compiler::expression::Expr {
        let fns = crate::stdlib::all();
        let compiled =
            crate::compiler::compile(source, &fns).expect("test expression should compile");
        compiled
            .program
            .expressions
            .into_inner()
            .pop()
            .expect("program should have at least one expression")
    }

    fn compile_get_fn_for_test(
        value_expr: crate::compiler::expression::Expr,
        path_expr: crate::compiler::expression::Expr,
    ) -> GetFn {
        let mut args = ArgumentList::default();
        args.arguments.insert("value", value_expr);
        args.arguments.insert("path", path_expr);
        GetFn::compile(&args)
    }

    #[test]
    fn compile_classifies_value_sources_and_const_paths() {
        let valid_path = compile_expr(r#"["a", 0]"#);
        let invalid_path = compile_expr(r#"["a", true]"#);

        let ext_event = compile_get_fn_for_test(compile_expr("."), valid_path.clone());
        assert!(matches!(ext_event.value, ValueSource::External(_)));
        assert!(matches!(ext_event.path, ConstOrExpr::Const(_)));
        assert_eq!(
            ext_event.type_def(&state::TypeState::default()),
            TypeDef::any().fallible()
        );

        let ext_meta = compile_get_fn_for_test(compile_expr("%sub"), invalid_path);
        assert!(matches!(ext_meta.value, ValueSource::External(_)));
        assert!(matches!(ext_meta.path, ConstOrExpr::Expr(_)));

        let var_root =
            compile_get_fn_for_test(compile_expr("var = {\"a\": 1}; var"), valid_path.clone());
        assert!(matches!(var_root.value, ValueSource::Variable { .. }));

        let var_sub = compile_get_fn_for_test(
            compile_expr("var = {\"sub\": {\"a\": 1}}; var.sub"),
            valid_path.clone(),
        );
        assert!(matches!(var_sub.value, ValueSource::Variable { .. }));

        let owned_container_query =
            compile_get_fn_for_test(compile_expr(r#"{"a": {"b": 1}}.a"#), valid_path.clone());
        assert!(matches!(
            owned_container_query.value,
            ValueSource::OwnedQuery { .. }
        ));

        let owned_fn_query =
            compile_get_fn_for_test(compile_expr(r#"parse_json!("{}").a"#), valid_path.clone());
        assert!(matches!(
            owned_fn_query.value,
            ValueSource::OwnedQuery { .. }
        ));

        let owned_literal = compile_get_fn_for_test(compile_expr(r#"{"a": 1}"#), valid_path);
        assert!(matches!(owned_literal.value, ValueSource::Owned(_)));
    }

    #[test]
    fn external_event_and_metadata_targets() {
        let target = value!({"nested": {"deep": {"leaf": 42}}});
        let metadata = value!({"meta": {"key": "secret"}});

        assert_eq!(
            run_vrl(
                r#"get!(., ["nested", "deep", "leaf"])"#,
                target.clone(),
                metadata.clone()
            ),
            Ok(value!(42))
        );
        assert_eq!(
            run_vrl(
                r#"get!(.nested, ["deep", "leaf"])"#,
                target.clone(),
                metadata.clone()
            ),
            Ok(value!(42))
        );
        assert_eq!(
            run_vrl("get!(., [])", target.clone(), metadata.clone()),
            Ok(target.clone())
        );
        assert_eq!(
            run_vrl(r#"get!(., ["missing"])"#, target.clone(), metadata.clone()),
            Ok(Value::Null)
        );
        assert_eq!(
            run_vrl(
                r#"get!(%, ["meta", "key"])"#,
                target.clone(),
                metadata.clone()
            ),
            Ok(value!("secret"))
        );
        assert_eq!(
            run_vrl(r#"get!(%meta, ["key"])"#, target.clone(), metadata.clone()),
            Ok(value!("secret"))
        );
        assert_eq!(
            run_vrl(r#"get!(.missing, ["leaf"])"#, target, metadata),
            Ok(Value::Null)
        );
    }

    #[test]
    fn local_variables_and_subpaths() {
        assert_eq!(
            run_vrl(
                r#"var = {"nested": {"deep": "ok"}}; get!(var, ["nested", "deep"])"#,
                value!({}),
                value!({})
            ),
            Ok(value!("ok"))
        );
        assert_eq!(
            run_vrl(
                r#"var = {"nested": {"deep": "ok"}}; get!(var.nested, ["deep"])"#,
                value!({}),
                value!({})
            ),
            Ok(value!("ok"))
        );
        assert_eq!(
            run_vrl(
                r#"var = {"nested": 1}; get!(var, [])"#,
                value!({}),
                value!({})
            ),
            Ok(value!({"nested": 1}))
        );
        assert_eq!(
            run_vrl(
                r#"var = .; get!(var.missing, ["x"])"#,
                value!({"nested": 1}),
                value!({})
            ),
            Ok(Value::Null)
        );
    }

    #[test]
    fn owned_temporaries_and_owned_queries() {
        assert_eq!(
            run_vrl(
                r#"get!({"a": {"b": [10, 20, 30]}}, ["a", "b", -2])"#,
                value!({}),
                value!({})
            ),
            Ok(value!(20))
        );
        assert_eq!(
            run_vrl(r#"get!({"a": 1}, [])"#, value!({}), value!({})),
            Ok(value!({"a": 1}))
        );
        assert_eq!(
            run_vrl(r#"get!({"a": {"b": 7}}.a, ["b"])"#, value!({}), value!({})),
            Ok(value!(7))
        );
        assert_eq!(
            run_vrl(
                r#"get!(parse_json!("{\"a\":{\"b\":42}}").a, ["b"])"#,
                value!({}),
                value!({})
            ),
            Ok(value!(42))
        );
        assert_eq!(
            run_vrl("get!([0, 1, 2, 3, 4], [2])", value!({}), value!({})),
            Ok(value!(2))
        );
    }

    #[test]
    fn evaluation_order_and_side_effects() {
        assert_eq!(
            run_vrl(
                r#"get!(., [{ .x = "updated"; "x" }])"#,
                value!({"x": "initial"}),
                value!({})
            ),
            Ok(value!("updated"))
        );
        assert_eq!(
            run_vrl(
                r#"var = {"x": "old"}; get!(var, [{ var.x = "new"; "x" }])"#,
                value!({}),
                value!({})
            ),
            Ok(value!("new"))
        );

        let err = run_vrl(
            r#"get!(parse_json!("not json"), ["foo", true])"#,
            value!({}),
            value!({}),
        )
        .expect_err("fallible value must fail before invalid path validation");
        assert!(
            err.contains("unable to parse json"),
            "expected parse_json error, got: {err}"
        );
        assert!(
            !err.contains("path segment must be"),
            "unexpected path segment error: {err}"
        );
    }

    #[test]
    fn captured_path_variable_reassigned_in_closure() {
        assert_eq!(
            run_vrl(
                r#"p = ["a"]; for_each([0, 1]) -> |_i, _v| { .result = get!({"a": 1, "b": 2}, p); p = ["b"] }; .result"#,
                value!({}),
                value!({})
            ),
            Ok(value!(2))
        );
        assert_eq!(
            run_vrl(
                r#"k = "a"; for_each([0, 1]) -> |_i, _v| { .result = get!({"a": 1, "b": 2}, [k]); k = "b" }; .result"#,
                value!({}),
                value!({})
            ),
            Ok(value!(2))
        );
    }

    #[test]
    fn error_semantics_and_edge_cases() {
        let invalid_const = run_vrl(
            r#"get!({"foo": "bar"}, ["foo", true])"#,
            value!({}),
            value!({}),
        )
        .expect_err("invalid const path must fail at runtime");
        assert!(
            invalid_const.contains("path segment must be either string or integer, not boolean"),
            "unexpected error: {invalid_const}"
        );

        let trailing_invalid = run_vrl(
            r#"p = ["missing", true]; get!({"foo": "bar"}, p)"#,
            value!({}),
            value!({}),
        )
        .expect_err("invalid trailing segment after missing prefix must fail");
        assert!(
            trailing_invalid.contains("path segment must be either string or integer, not boolean"),
            "unexpected error: {trailing_invalid}"
        );

        let non_array = run_vrl(
            r#"p = .dyn_path; get!({"foo": 1}, p)"#,
            value!({"dyn_path": "foo"}),
            value!({}),
        )
        .expect_err("non-array dynamic path must fail");
        assert!(
            non_array.contains("expected array"),
            "unexpected error: {non_array}"
        );

        assert_eq!(
            run_vrl(
                r#"v = .scalar; get!(v, ["a"])"#,
                value!({"scalar": 42}),
                value!({})
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            run_vrl(
                r#"get!(.scalar, ["a"])"#,
                value!({"scalar": 42}),
                value!({})
            ),
            Ok(Value::Null)
        );

        // Non-UTF-8 bytes in dynamic path and constant path
        let target_non_utf8 = value!({"\u{FFFD}": "found"});
        let bad_bytes = Value::Bytes(bytes::Bytes::from_static(b"\xff"));
        assert_eq!(
            run_vrl(
                "p = [.bad]; get!(., p)",
                value!({"\u{FFFD}": "found", "bad": bad_bytes}),
                value!({})
            ),
            Ok(value!("found"))
        );
        let const_non_utf8_path = Expr::Container(crate::compiler::expression::Container::new(
            crate::compiler::expression::container::Variant::Array(
                crate::compiler::expression::Array::new(vec![Expr::Literal(
                    bytes::Bytes::from_static(b"\xff").into(),
                )]),
            ),
        ));
        let const_non_utf8_fn = compile_get_fn_for_test(compile_expr("."), const_non_utf8_path);
        assert!(matches!(const_non_utf8_fn.path, ConstOrExpr::Const(_)));
        let mut tv = crate::compiler::TargetValue {
            value: target_non_utf8,
            metadata: value!({}),
            secrets: crate::value::Secrets::default(),
        };
        let mut rstate = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = Context::new(&mut tv, &mut rstate, &tz);
        assert_eq!(const_non_utf8_fn.resolve(&mut ctx), Ok(value!("found")));

        // Out-of-bounds negative indices
        assert_eq!(
            run_vrl(
                "get!([1, 2, 3], [-9223372036854775808])",
                value!({}),
                value!({})
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            run_vrl("get!([1, 2, 3], [-1000])", value!({}), value!({})),
            Ok(Value::Null)
        );

        // TypeDef stays TypeDef::any().fallible()
        let fn_expr = compile_get_fn_for_test(compile_expr("."), compile_expr(r#"["a"]"#));
        assert_eq!(
            fn_expr.type_def(&state::TypeState::default()),
            TypeDef::any().fallible()
        );
    }
}
