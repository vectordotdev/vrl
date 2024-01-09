use std::collections::BTreeMap;

use crate::compiler::prelude::*;

fn replace_with<T>(
    value: Value,
    pattern: Value,
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
    match pattern {
        Value::Regex(regex) => {
            let captures = regex.captures_iter(&haystack);
            make_replacement(captures, &haystack, count, ctx, runner)
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::regex(),
        }
        .into()),
    }
}

fn make_replacement<T>(
    caps: regex::CaptureMatches,
    haystack: &str,
    count: usize,
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
    for (i, cap) in caps.enumerate() {
        // Safe to unrap because the 0th index always includes the full match.
        let m = cap.get(0).unwrap(); // full match

        let mut value = captures_to_value(&cap);
        runner.map_value(ctx, &mut value)?;
        let replacement = value.try_bytes_utf8_lossy()?;

        replaced.push_str(&haystack[last_match..m.start()]);
        replaced.push_str(&replacement);
        last_match = m.end();
        if i >= limit {
            break;
        }
    }
    // add the final component
    replaced.push_str(&haystack[last_match..]);
    Ok(replaced.into())
}

fn captures_to_value(captures: &regex::Captures) -> Value {
    // return an array of the capture groups
    captures
        .iter()
        .map(|m| match m {
            Some(m) => m.as_str().into(),
            None => Value::Null,
        })
        .collect()
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
                source: r#"replace_with("foobar", r'o|a') -> |m| { m[0] + m[0] }"#,
                result: Ok("foooobaar"),
            },
            Example {
                title: "replace count",
                source: r#"replace_with("foobar", r'o|a', count: 1) -> |m| { m[0] + m[0] }"#,
                result: Ok("fooobar"),
            },
            Example {
                title: "replace with capture group",
                source: r#"replace_with("foo123bar", r'foo(\d+)bar') -> |m| { x = m[1]; "x={{x}}" }"#,
                result: Ok(r#"x=123"#),
            },
            Example {
                title: "process capture group",
                source: r#"replace_with(s'Got message: {"msg": "b"}', r'message: (\{.*\})') -> |m| { to_string!(parse_json!(m[1]).msg) }"#,
                result: Ok("Got b"),
            },
            Example {
                title: "Optional capture group",
                source: r#"replace_with("foobar", r'bar( of gold)?') -> |m| { if m[1] == null { "baz" } else { "rich" } }"#,
                result: Ok("foobaz"),
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

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::bytes(),
                variables: vec![
                    Variable {
                        kind: VariableKind::Exact(Kind::array(Collection::from_parts(
                                          BTreeMap::from([(0.into(), Kind::bytes())]),
                                          Kind::bytes()))),
                    },
                ],
                output: Output::Kind(Kind::bytes()),
                example: Example {
                    title: "replace with hash",
                    source: r#"replace_with("received email from a@example.com", pattern: r'\w+@\w+\.\w+') -> |match| { sha2(match[0]) }"#,
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
