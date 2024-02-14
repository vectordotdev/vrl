use std::collections::BTreeMap;

use regex::{CaptureMatches, CaptureNames, Captures, Regex};

use crate::compiler::prelude::*;

fn replace_with<T>(
    value: Value,
    pattern: &Regex,
    count: Value,
    ctx: &mut Context,
    runner: closure::Runner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    let haystack = value.try_bytes_utf8_lossy()?;
    let count = match count.try_integer()? {
        i if i > 0 => i as usize,
        i if i < 0 => 0,
        // this is when i == 0
        _ => return Ok(value),
    };
    let captures = pattern.captures_iter(&haystack);
    make_replacement(
        captures,
        &haystack,
        count,
        pattern.capture_names(),
        ctx,
        runner,
    )
}

fn make_replacement<T>(
    caps: CaptureMatches,
    haystack: &str,
    count: usize,
    capture_names: CaptureNames,
    ctx: &mut Context,
    runner: closure::Runner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    // possible optimization: peek at first capture, if none return the original value.
    let mut replaced = String::with_capacity(haystack.len());
    let limit = if count == 0 { usize::MAX } else { count - 1 };
    let mut last_match = 0;
    // we loop over the matches ourselves instead of calling Regex::replacen, so that we can
    // handle errors. This is however based on the implementation of Regex::replacen
    for (idx, captures) in caps.enumerate() {
        // Safe to unrap because the 0th index always includes the full match.
        let m = captures.get(0).unwrap(); // full match

        let mut value = captures_to_value(&captures, capture_names.clone());
        runner.map_value(ctx, &mut value)?;
        let replacement = value.try_bytes_utf8_lossy()?;

        replaced.push_str(&haystack[last_match..m.start()]);
        replaced.push_str(&replacement);
        last_match = m.end();
        if idx >= limit {
            break;
        }
    }
    // add the final component
    replaced.push_str(&haystack[last_match..]);
    Ok(replaced.into())
}

const STRING_NAME: &str = "string";
const CAPTURES_NAME: &str = "captures";

fn captures_to_value(captures: &Captures, capture_names: CaptureNames) -> Value {
    let mut object: ObjectMap = BTreeMap::new();

    // The full match, named "string"
    object.insert(STRING_NAME.into(), captures.get(0).unwrap().as_str().into());
    // The length includes the total match, so subtract 1
    let mut capture_groups: Vec<Value> = Vec::with_capacity(captures.len() - 1);

    // We skip the first entry, because it is for the full match, which we have already
    // extracted
    for (idx, name) in capture_names.enumerate().skip(1) {
        let value: Value = if let Some(group) = captures.get(idx) {
            group.as_str().into()
        } else {
            Value::Null
        };
        if let Some(name) = name {
            object.insert(name.into(), value.clone());
        }
        capture_groups.push(value);
    }

    object.insert(CAPTURES_NAME.into(), capture_groups.into());

    object.into()
}

#[derive(Clone, Copy, Debug)]
pub struct ReplaceWith;

impl Function for ReplaceWith {
    fn identifier(&self) -> &'static str {
        "replace_with"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "pattern",
                kind: kind::REGEX,
                required: true,
            },
            Parameter {
                keyword: "count",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "double replacement",
                source: r#"replace_with("foobar", r'o|a') -> |m| { m.string + m.string }"#,
                result: Ok("foooobaar"),
            },
            Example {
                title: "replace count",
                source: r#"replace_with("foobar", r'o|a', count: 1) -> |m| { m.string + m.string }"#,
                result: Ok("fooobar"),
            },
            Example {
                title: "replace with capture group",
                source: r#"replace_with("foo123bar", r'foo(\d+)bar') -> |m| { x = m.captures[0]; "x={{x}}" }"#,
                result: Ok("x=123"),
            },
            Example {
                title: "process capture group",
                source: r#"replace_with(s'Got message: {"msg": "b"}', r'message: (\{.*\})') -> |m| { to_string!(parse_json!(m.captures[0]).msg) }"#,
                result: Ok("Got b"),
            },
            Example {
                title: "Optional capture group",
                source: r#"replace_with("foobar", r'bar( of gold)?') -> |m| { if m.captures[1] == null { "baz" } else { "rich" } }"#,
                result: Ok("foobaz"),
            },
            Example {
                title: "Named capture group",
                source: r#"replace_with("foo123bar", r'foo(?P<num>\d+)bar') -> |m| { x = to_int!(m.num); to_string(x+ 1) }"#, //to_string(to_int!(m.named.num) + 1) }"#,
                result: Ok("\"124\""),
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
        let pattern = arguments.required("pattern");
        let count = arguments.optional("count").unwrap_or(expr!(-1));

        let closure = arguments.required_closure()?;

        Ok(ReplaceWithFn {
            value,
            pattern,
            count,
            closure,
        }
        .as_expr())
    }

    fn closure(&self) -> Option<closure::Definition> {
        use closure::{Definition, Input, Output, Variable, VariableKind};

        let match_type = Collection::from_parts(
            BTreeMap::from([
                (STRING_NAME.into(), Kind::bytes()),
                (
                    CAPTURES_NAME.into(),
                    Kind::array(Collection::from_unknown(Kind::bytes().or_null())),
                ),
            ]),
            Kind::bytes().or_null(),
        );

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::bytes(),
                variables: vec![
                    Variable {
                        kind: VariableKind::Exact(Kind::object(match_type)),
                    },
                ],
                output: Output::Kind(Kind::bytes()),
                example: Example {
                    title: "replace with hash",
                    source: r#"replace_with("received email from a@example.com", pattern: r'\w+@\w+\.\w+') -> |match| { sha2(match.string) }"#,
                    result: Ok("received email from 896bdca840c9304a5d0bdbeacc4ef359e3093f80c9777c9967e31ba0ff99ed58"),
                },
            }],
            is_iterator: false,
        })
    }
}

#[derive(Debug, Clone)]
struct ReplaceWithFn {
    value: Box<dyn Expression>,
    pattern: Box<dyn Expression>,
    count: Box<dyn Expression>,
    closure: FunctionClosure,
}

impl FunctionExpression for ReplaceWithFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let value = self.value.resolve(ctx)?;
        let pattern = self.pattern.resolve(ctx)?;
        let pattern = pattern
            .as_regex()
            .ok_or_else(|| ExpressionError::from("failed to resolve regex"))?;
        for name in pattern.capture_names().flatten() {
            if name == STRING_NAME || name == CAPTURES_NAME {
                return Err(ExpressionError::from(
                    r#"Capture group cannot be named "string" or "captures""#,
                ));
            }
        }
        let count = self.count.resolve(ctx)?;
        let FunctionClosure {
            variables, block, ..
        } = &self.closure;

        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        replace_with(value, pattern, count, ctx, runner)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}
