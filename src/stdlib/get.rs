use crate::compiler::prelude::*;
use crate::path::{BorrowedSegment, OwnedSegment, OwnedValuePath, ValuePath};

#[allow(dead_code)] // Wired into GetFn in Task 2
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

#[allow(dead_code)] // Wired into GetFn in Task 2
#[derive(Clone, Copy)]
struct BorrowedValuePath<'a>(&'a [Value]);

#[allow(dead_code)] // Wired into GetFn in Task 2
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

#[allow(dead_code)] // Wired into Get::compile in Task 2
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
        let value = arguments.required("value");
        let path = arguments.required("path");

        Ok(GetFn { value, path }.as_expr())
    }
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
}
