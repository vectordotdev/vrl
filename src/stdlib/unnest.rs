use crate::compiler::prelude::*;
use crate::path::{OwnedSegment, OwnedTargetPath, OwnedValuePath};

fn unnest(path: &expression::Query, ctx: &mut Context) -> Resolved {
    let lookup_buf = path.path();

    match path.target() {
        expression::Target::External(prefix) => {
            let root = ctx
                .target()
                .target_get(&OwnedTargetPath::root(*prefix))?
                .unwrap_or(&Value::Null);
            unnest_borrowed(root, lookup_buf)
        }
        expression::Target::Internal(v) => {
            let root = ctx.state().variable(v.ident()).unwrap_or(&Value::Null);
            unnest_borrowed(root, lookup_buf)
        }
        expression::Target::Container(expr) => {
            let value = expr.resolve(ctx)?;
            unnest_owned(value, lookup_buf)
        }
        expression::Target::FunctionCall(expr) => {
            let value = expr.resolve(ctx)?;
            unnest_owned(value, lookup_buf)
        }
    }
}

fn validate_array<'a>(root: &'a Value, path: &OwnedValuePath) -> Result<&'a [Value], ValueError> {
    match root.get(path) {
        Some(Value::Array(arr)) => Ok(arr),
        Some(other) => Err(ValueError::Expected {
            got: other.kind(),
            expected: Kind::array(Collection::any()),
        }),
        None => Err(ValueError::Expected {
            got: Kind::null(),
            expected: Kind::array(Collection::any()),
        }),
    }
}

fn unnest_borrowed(root: &Value, path: &OwnedValuePath) -> Resolved {
    let values = validate_array(root, path)?;
    if values.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    if path.segments.is_empty() {
        return Ok(Value::Array(values.to_vec()));
    }
    unnest_owned(root.clone(), path)
}

fn unnest_owned(mut root: Value, path: &OwnedValuePath) -> Resolved {
    let len = validate_array(&root, path)?.len();
    if len == 0 {
        return Ok(Value::Array(Vec::new()));
    }
    if path.segments.is_empty() {
        return Ok(root);
    }

    let has_index = path
        .segments
        .iter()
        .any(|s| matches!(s, OwnedSegment::Index(_)));

    if !has_index {
        let Some(slot) = root.get_mut(path) else {
            return Err(ValueError::Expected {
                got: Kind::null(),
                expected: Kind::array(Collection::any()),
            }
            .into());
        };
        let mut values = match std::mem::replace(slot, Value::Null) {
            Value::Array(values) => values,
            other => {
                return Err(ValueError::Expected {
                    got: other.kind(),
                    expected: Kind::array(Collection::any()),
                }
                .into());
            }
        };

        let Some(last) = values.pop() else {
            return Ok(Value::Array(Vec::new()));
        };

        let mut events = Vec::with_capacity(len);
        for value in values {
            let mut event = root.clone();
            if let Some(slot) = event.get_mut(path) {
                *slot = value;
            }
            events.push(event);
        }

        if let Some(slot) = root.get_mut(path) {
            *slot = last;
        }
        events.push(root);

        return Ok(Value::Array(events));
    }

    let mut values = root
        .remove(path, true)
        .ok_or(ValueError::Expected {
            got: Kind::null(),
            expected: Kind::array(Collection::any()),
        })?
        .try_array()?;

    let Some(last) = values.pop() else {
        return Ok(Value::Array(Vec::new()));
    };

    let mut events = Vec::with_capacity(len);
    for value in values {
        let mut event = root.clone();
        event.insert(path, value);
        events.push(event);
    }

    root.insert(path, last);
    events.push(root);

    Ok(Value::Array(events))
}

#[derive(Clone, Copy, Debug)]
pub struct Unnest;

impl Function for Unnest {
    fn identifier(&self) -> &'static str {
        "unnest"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Unnest an array field from an object to create an array of objects using that field; keeping all other fields.

            Assigning the array result of this to `.` results in multiple events being emitted from `remap`. See the
            [`remap` transform docs](/docs/reference/configuration/transforms/remap/#emitting-multiple-log-events) for more details.

            This is also referred to as `explode` in some languages.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Object.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["The field path referred to is not an array."]
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns an array of objects that matches the original object, but each with the specified path replaced with a single element from the original path.",
            "When `path` traverses array indices (for example, `.a[0].events` or `.a[0]`), empty intermediate containers are pruned before each element is reinserted at `path`, which may shift or overwrite subsequent array elements.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "path",
            kind::ARRAY,
            "The path of the field to unnest.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Unnest an array field",
                source: indoc! {r#"
                    . = {"hostname": "localhost", "messages": ["message 1", "message 2"]}
                    . = unnest(.messages)
                "#},
                result: Ok(
                    r#"[{"hostname": "localhost", "messages": "message 1"}, {"hostname": "localhost", "messages": "message 2"}]"#,
                ),
            },
            example! {
                title: "Unnest a nested array field",
                source: indoc! {r#"
                    . = {"hostname": "localhost", "event": {"messages": ["message 1", "message 2"]}}
                    . = unnest(.event.messages)
                "#},
                result: Ok(
                    r#"[{"hostname": "localhost", "event": {"messages": "message 1"}}, {"hostname": "localhost", "event": {"messages": "message 2"}}]"#,
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
        let path = arguments.required_query("path")?;
        Ok(UnnestFn { path }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UnnestFn {
    path: expression::Query,
}

impl UnnestFn {
    #[cfg(test)]
    fn new(path: &str) -> Self {
        use crate::path::{PathPrefix, parse_value_path};

        Self {
            path: expression::Query::new(
                expression::Target::External(PathPrefix::Event),
                parse_value_path(path).unwrap(),
            ),
        }
    }
}

impl FunctionExpression for UnnestFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        unnest(&self.path, ctx)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        use expression::Target;

        match self.path.target() {
            Target::External(prefix) => invert_array_at_path(
                &TypeDef::from(state.external.kind(*prefix)),
                self.path.path(),
            ),
            Target::Internal(v) => invert_array_at_path(&v.type_def(state), self.path.path()),
            Target::FunctionCall(f) => invert_array_at_path(&f.type_def(state), self.path.path()),
            Target::Container(c) => invert_array_at_path(&c.type_def(state), self.path.path()),
        }
    }
}

/// Assuming path points at an Array, this will take the typedefs for that array,
/// And will remove it returning a set of it's elements.
///
/// For example the typedef for this object:
/// `{ "a" => { "b" => [ { "c" => 2 }, { "c" => 3 } ] } }`
///
/// Is converted to a typedef for this array:
/// `[ { "a" => { "b" => { "c" => 2 } } },
///    { "a" => { "b" => { "c" => 3 } } },
///  ]`
///
pub(crate) fn invert_array_at_path(typedef: &TypeDef, path: &OwnedValuePath) -> TypeDef {
    let kind = typedef.kind().at_path(path);

    let Some(mut array) = kind.into_array() else {
        // Guaranteed fallible.
        // This can't actually be set to "fallible", or it will cause problems due to
        // https://github.com/vectordotdev/vector/issues/13527
        return TypeDef::never();
    };

    array.known_mut().values_mut().for_each(|kind| {
        let mut tdkind = typedef.kind().clone();
        tdkind.insert(path, kind.clone());

        *kind = tdkind.clone();
    });

    let unknown = array.unknown_kind();
    if unknown.contains_any_defined() {
        let mut tdkind = typedef.kind().clone();
        tdkind.insert(path, unknown.without_undefined());
        array.set_unknown(tdkind);
    }

    TypeDef::array(array).infallible()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::parse_value_path;
    use crate::{btreemap, type_def, value};

    #[test]
    #[allow(clippy::too_many_lines)]
    fn type_def() {
        struct TestCase {
            old: TypeDef,
            path: &'static str,
            new: TypeDef,
        }

        let cases = vec![
            // Simple case
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { array [
                        type_def! { object {
                            "noog" => type_def! { bytes },
                            "nork" => type_def! { bytes },
                        } },
                    ] },
                } },
                path: ".nonk",
                new: type_def! { array [
                    type_def! { object {
                        "nonk" => type_def! { object {
                            "noog" => type_def! { bytes },
                            "nork" => type_def! { bytes },
                        } },
                    } },
                ] },
            },
            // Provided example
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { object {
                        "shnoog" => type_def! { array [
                            type_def! { object {
                                "noog" => type_def! { bytes },
                            } },
                        ] },
                    } },
                } },
                path: "nonk.shnoog",
                new: type_def! { array [
                    type_def! { object {
                        "nonk" => type_def! { object {
                            "shnoog" => type_def! { object {
                                "noog" => type_def! { bytes },
                            } },
                        } },
                    } },
                ] },
            },
            // Same field in different branches
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { object {
                        "shnoog" => type_def! { array [
                            type_def! { object {
                                "noog" => type_def! { bytes },
                            } },
                        ] },
                    } },
                    "nink" => type_def! { object {
                        "shnoog" => type_def! { array [
                            type_def! { object {
                                "noog" => type_def! { bytes },
                            } },
                        ] },
                    } },
                } },
                path: "nonk.shnoog",
                new: type_def! { array [
                    type_def! { object {
                        "nonk" => type_def! { object {
                            "shnoog" => type_def! { object {
                                "noog" => type_def! { bytes },
                            } },
                        } },
                        "nink" => type_def! { object {
                            "shnoog" => type_def! { array [
                                type_def! { object {
                                    "noog" => type_def! { bytes },
                                } },
                            ] },
                        } },
                    } },
                ] },
            },
            // Indexed specific
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { array {
                        0 => type_def! { object {
                            "noog" => type_def! { array [
                                type_def! { bytes },
                            ] },
                            "nork" => type_def! { bytes },
                        } },
                    } },
                } },
                path: ".nonk[0].noog",
                new: type_def! { array [
                    type_def! { object {
                        "nonk" => type_def! { array {
                            // The index is added on top of the Any entry.
                            0 => type_def! { object {
                                "noog" => type_def! { bytes },
                                "nork" => type_def! { bytes },
                            } },
                        } },
                    } },
                ] },
            },
            // More nested
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { object {
                        "shnoog" => type_def! { array [
                            type_def! { object {
                                "noog" => type_def! { bytes },
                                "nork" => type_def! { bytes },
                            } },
                        ] },
                    } },
                } },
                path: ".nonk.shnoog",
                new: type_def! { array [
                    type_def! { object {
                        "nonk" => type_def! { object {
                            "shnoog" => type_def! { object {
                                "noog" => type_def! { bytes },
                                "nork" => type_def! { bytes },
                            } },
                        } },
                    } },
                ] },
            },
            // Nonexistent, the types we know are moved into the returned array.
            TestCase {
                old: type_def! { object {
                    "nonk" => type_def! { bytes },
                } },
                path: ".norg",
                // guaranteed to fail at runtime
                new: TypeDef::never(),
            },
        ];

        for case in cases {
            let path = parse_value_path(case.path).unwrap();
            let new = invert_array_at_path(&case.old, &path);
            assert_eq!(case.new, new, "{path}");
        }
    }

    #[test]
    fn unnest() {
        let cases = vec![
            (
                value!({"hostname": "localhost", "events": [{"message": "hello"}, {"message": "world"}]}),
                Ok(
                    value!([{"hostname": "localhost", "events": {"message": "hello"}}, {"hostname": "localhost", "events": {"message": "world"}}]),
                ),
                UnnestFn::new("events"),
                type_def! { array [
                    type_def! { object {
                        "hostname" => type_def! { bytes },
                        "events" => type_def! { object {
                            "message" => type_def! { bytes },
                        } },
                    } },
                ] },
            ),
            (
                value!({"hostname": "localhost", "events": [{"message": "hello"}, {"message": "world"}]}),
                Err("expected array, got null".to_owned()),
                UnnestFn::new("unknown"),
                // guaranteed to always fail
                TypeDef::never(),
            ),
            (
                value!({"hostname": "localhost", "events": [{"message": "hello"}, {"message": "world"}]}),
                Err("expected array, got string".to_owned()),
                UnnestFn::new("hostname"),
                // guaranteed to always fail
                TypeDef::never(),
            ),
        ];

        let local = state::LocalEnv::default();
        let external = state::ExternalEnv::new_with_kind(
            Kind::object(btreemap! {
                "hostname" => Kind::bytes(),
                "events" => Kind::array(Collection::from_unknown(Kind::object(btreemap! {
                    Field::from("message") => Kind::bytes(),
                })),
            )}),
            Kind::object(Collection::empty()),
        );
        let state = TypeState { local, external };

        let tz = TimeZone::default();
        for (object, expected, func, expected_typedef) in cases {
            let mut object = object.clone();
            let mut runtime_state = state::RuntimeState::default();
            let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);

            let got_typedef = func.type_def(&state);

            let got = func
                .resolve(&mut ctx)
                .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));

            assert_eq!(got, expected);
            assert_eq!(got_typedef, expected_typedef);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unnest_edge_cases() {
        use crate::compiler::{Span, compile, parser::ast::Ident, type_def::Details};

        let tz = TimeZone::default();

        // External target edge cases: empty/single arrays, deep pure-Field paths,
        // root path, null/missing fields, and indexed paths (prune-and-reinsert).
        let external_cases = vec![
            (
                ".events",
                value!({"host": "a", "events": []}),
                Ok(value!([])),
            ),
            (
                ".events",
                value!({"host": "a", "events": [1]}),
                Ok(value!([{"host": "a", "events": 1}])),
            ),
            (
                ".a.b.events",
                value!({"host": "a", "a": {"keep": 1, "b": {"events": [1, 2]}}}),
                Ok(value!([
                    {"host": "a", "a": {"keep": 1, "b": {"events": 1}}},
                    {"host": "a", "a": {"keep": 1, "b": {"events": 2}}},
                ])),
            ),
            (
                ".a.b.events",
                value!({"a": {"b": {"events": [1, 2]}}}),
                Ok(value!([
                    {"a": {"b": {"events": 1}}},
                    {"a": {"b": {"events": 2}}},
                ])),
            ),
            (".", value!([1, 2]), Ok(value!([1, 2]))),
            (".", value!([]), Ok(value!([]))),
            (
                ".",
                value!({"a": 1}),
                Err("expected array, got { a: integer }".to_owned()),
            ),
            (
                ".events",
                value!({"events": null}),
                Err("expected array, got null".to_owned()),
            ),
            (
                ".events",
                value!({}),
                Err("expected array, got null".to_owned()),
            ),
            (
                ".a[0].events",
                value!({"a": [{"events": [1, 2]}, {"other": 3}]}),
                Ok(value!([
                    {"a": [{"other": 3, "events": 1}]},
                    {"a": [{"other": 3, "events": 2}]},
                ])),
            ),
            (
                ".a[0]",
                value!({"a": [[1, 2], [3, 4]]}),
                Ok(value!([{"a": [1]}, {"a": [2]}])),
            ),
            (
                ".a[0].events",
                value!({"a": [{"events": [1], "keep": true}]}),
                Ok(value!([{"a": [{"events": 1, "keep": true}]}])),
            ),
            (
                ".a[-1].events",
                value!({"a": [{"other": 1}, {"events": [2, 3], "keep": true}]}),
                Ok(value!([
                    {"a": [{"other": 1}, {"events": 2, "keep": true}]},
                    {"a": [{"other": 1}, {"events": 3, "keep": true}]},
                ])),
            ),
        ];

        for (path, mut target, expected) in external_cases {
            let func = UnnestFn::new(path);
            let mut runtime_state = state::RuntimeState::default();
            let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);
            let got = func
                .resolve(&mut ctx)
                .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));
            assert_eq!(got, expected, "path={path}");
        }

        // Target::Internal (set variable and unset variable).
        let ident = Ident::new("v".to_owned());
        let mut local = state::LocalEnv::default();
        local.insert_variable(
            ident.clone(),
            Details {
                type_def: TypeDef::any().infallible(),
                value: None,
            },
        );
        let var = expression::Variable::new(Span::new(0, 1), ident.clone(), &local).unwrap();
        let internal_func = UnnestFn {
            path: expression::Query::new(
                expression::Target::Internal(var),
                parse_value_path("events").unwrap(),
            ),
        };

        let mut target = value!({});
        let mut runtime_state = state::RuntimeState::default();
        runtime_state.insert_variable(ident, value!({"a": 1, "events": [1, 2]}));
        let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);
        let got = internal_func
            .resolve(&mut ctx)
            .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));
        assert_eq!(
            got,
            Ok(value!([{"a": 1, "events": 1}, {"a": 1, "events": 2}])),
            "Target::Internal (set)"
        );

        let mut target = value!({});
        let mut empty_state = state::RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut empty_state, &tz);
        let got = internal_func
            .resolve(&mut ctx)
            .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));
        assert_eq!(
            got,
            Err("expected array, got null".to_owned()),
            "Target::Internal (unset)"
        );

        // Target::Container and Target::FunctionCall via compiled VRL.
        let fns: Vec<Box<dyn Function>> =
            vec![Box::new(Unnest), Box::new(crate::stdlib::merge::Merge)];

        for (label, src) in [
            (
                "Target::Container",
                r#"unnest({"a": 1, "events": [1, 2]}.events)"#,
            ),
            (
                "Target::FunctionCall",
                r#"unnest(merge({"a": 1}, {"events": [1, 2]}).events)"#,
            ),
        ] {
            let compiled = compile(src, &fns).expect("program compiles");
            let mut target = value!({});
            let mut runtime_state = state::RuntimeState::default();
            let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);
            let got = compiled
                .program
                .resolve(&mut ctx)
                .expect("program resolves");
            assert_eq!(
                got,
                value!([{"a": 1, "events": 1}, {"a": 1, "events": 2}]),
                "{label}"
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unnest_borrowed_and_owned_direct() {
        let expected_null_err: Resolved = Err(ValueError::Expected {
            got: Kind::null(),
            expected: Kind::array(Collection::any()),
        }
        .into());
        let expected_bytes_err: Resolved = Err(ValueError::Expected {
            got: Kind::bytes(),
            expected: Kind::array(Collection::any()),
        }
        .into());

        let cases: Vec<(&str, Value, Resolved)> = vec![
            // Missing path and null value -> Err with Kind::null()
            (".events", value!({"host": "a"}), expected_null_err.clone()),
            (".events", value!({"events": null}), expected_null_err),
            // Non-array value -> Err with Kind::bytes()
            (
                ".events",
                value!({"events": "not-an-array"}),
                expected_bytes_err,
            ),
            // Empty array -> Ok(Value::Array(vec![]))
            (
                ".events",
                value!({"host": "a", "events": []}),
                Ok(Value::Array(vec![])),
            ),
            // Root path `.` on `[1, 2]` -> Ok(value!([1, 2]))
            (".", value!([1, 2]), Ok(value!([1, 2]))),
            // Pure-Field nested path with N = 1 and N = 3 (with siblings)
            (
                ".a.b.events",
                value!({"host": "h", "a": {"keep": 1, "b": {"events": [10]}}}),
                Ok(value!([{"host": "h", "a": {"keep": 1, "b": {"events": 10}}}])),
            ),
            (
                ".a.b.events",
                value!({"host": "h", "a": {"keep": 1, "b": {"events": [1, 2, 3]}}}),
                Ok(value!([
                    {"host": "h", "a": {"keep": 1, "b": {"events": 1}}},
                    {"host": "h", "a": {"keep": 1, "b": {"events": 2}}},
                    {"host": "h", "a": {"keep": 1, "b": {"events": 3}}},
                ])),
            ),
            // Pure-Field nested path with no siblings (N = 1 and N = 3)
            (
                ".a.b.events",
                value!({"a": {"b": {"events": [42]}}}),
                Ok(value!([{"a": {"b": {"events": 42}}}])),
            ),
            (
                ".a.b.events",
                value!({"a": {"b": {"events": [1, 2, 3]}}}),
                Ok(value!([
                    {"a": {"b": {"events": 1}}},
                    {"a": {"b": {"events": 2}}},
                    {"a": {"b": {"events": 3}}},
                ])),
            ),
            // Indexed paths (.a[0].events, .a[0], .a[-1].events) with N = 1 and N = 2
            (
                ".a[0].events",
                value!({"a": [{"events": [1], "keep": true}]}),
                Ok(value!([{"a": [{"events": 1, "keep": true}]}])),
            ),
            (
                ".a[0].events",
                value!({"a": [{"events": [1, 2]}, {"other": 3}]}),
                Ok(value!([
                    {"a": [{"other": 3, "events": 1}]},
                    {"a": [{"other": 3, "events": 2}]},
                ])),
            ),
            (
                ".a[0]",
                value!({"a": [[1], [2, 3]]}),
                Ok(value!([{"a": [1]}])),
            ),
            (
                ".a[0]",
                value!({"a": [[1, 2], [3, 4]]}),
                Ok(value!([{"a": [1]}, {"a": [2]}])),
            ),
            (
                ".a[-1].events",
                value!({"a": [{"other": 1}, {"events": [2], "keep": true}]}),
                Ok(value!([{"a": [{"other": 1}, {"events": 2, "keep": true}]}])),
            ),
            (
                ".a[-1].events",
                value!({"a": [{"other": 1}, {"events": [2, 3], "keep": true}]}),
                Ok(value!([
                    {"a": [{"other": 1}, {"events": 2, "keep": true}]},
                    {"a": [{"other": 1}, {"events": 3, "keep": true}]},
                ])),
            ),
        ];

        for (path_str, input, expected) in cases {
            let path = parse_value_path(path_str).unwrap();
            let got_borrowed = unnest_borrowed(&input, &path);
            let got_owned = unnest_owned(input, &path);
            assert_eq!(got_borrowed, expected, "unnest_borrowed path={path_str}");
            assert_eq!(got_owned, expected, "unnest_owned path={path_str}");
        }
    }
}
