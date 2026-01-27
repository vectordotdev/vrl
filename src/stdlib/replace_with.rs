use std::collections::BTreeMap;

use regex::{CaptureMatches, CaptureNames, Captures, Regex};

use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_COUNT: LazyLock<Value> = LazyLock::new(|| Value::Integer(-1));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The original string.",
            default: None,
        },
        Parameter {
            keyword: "pattern",
            kind: kind::REGEX,
            required: true,
            description: "Replace all matches of this pattern. Must be a regular expression.",
            default: None,
        },
        Parameter {
            keyword: "count",
            kind: kind::INTEGER,
            required: false,
            description: "The maximum number of replacements to perform. `-1` means replace all matches.",
            default: Some(&DEFAULT_COUNT),
        },
    ]
});

fn replace_with<T>(
    value: Value,
    pattern: &Regex,
    count: Value,
    ctx: &mut Context,
    runner: &closure::Runner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    let haystack = value.try_bytes_utf8_lossy()?;
    let count = match count.try_integer()? {
        // TODO consider removal options
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
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
        &pattern.capture_names(),
        ctx,
        runner,
    )
}

fn make_replacement<T>(
    caps: CaptureMatches,
    haystack: &str,
    count: usize,
    capture_names: &CaptureNames,
    ctx: &mut Context,
    runner: &closure::Runner<T>,
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

    fn usage(&self) -> &'static str {
        indoc! {"
            Replaces all matching instances of `pattern` using a closure.

            The `pattern` argument accepts a regular expression that can use capture groups.

            The function uses the function closure syntax to compute the replacement values.

            The closure takes a single parameter, which is an array, where the first item is always
            present and contains the entire string that matched `pattern`. The items from index one on
            contain the capture groups of the corresponding index. If a capture group is optional, the
            value may be null if it didn't match.

            The value returned by the closure must be a string and will replace the section of
            the input that was matched.

            This returns a new string with the replacements, the original string is not mutated.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Capitalize words",
                source: indoc! {r#"
                    replace_with("apples and bananas", r'\b(\w)(\w*)') -> |match| {
                        upcase!(match.captures[0]) + string!(match.captures[1])
                    }
                "#},
                result: Ok("Apples And Bananas"),
            },
            example! {
                title: "Replace with hash",
                source: indoc! {r#"
                    replace_with("email from test@example.com", r'\w+@example.com') -> |match| {
                        sha2(match.string, variant: "SHA-512/224")
                    }
                "#},
                result: Ok("email from adf6e1bc4415d24912bd93072ad34ef825a7b6eb3bf53f68def1fc17"),
            },
            example! {
                title: "Replace first instance",
                source: indoc! {r#"
                    replace_with("Apples and Apples", r'(?i)apples|cones', count: 1) -> |match| {
                        "Pine" + downcase(match.string)
                    }
                "#},
                result: Ok("Pineapples and Apples"),
            },
            example! {
                title: "Named capture group",
                source: indoc! {r#"
                    replace_with("level=error A message", r'level=(?P<level>\w+)') -> |match| {
                        lvl = upcase!(match.level)
                        "[{{lvl}}]"
                    }
                "#},
                result: Ok("[ERROR] A message"),
            },
            example! {
                title: "Replace with processed capture group",
                source: r#"replace_with(s'Got message: {"msg": "b"}', r'message: (\{.*\})') -> |m| { to_string!(parse_json!(m.captures[0]).msg) }"#,
                result: Ok("Got b"),
            },
            example! {
                title: "Replace with optional capture group",
                source: r#"replace_with("bar of chocolate and bar of gold", r'bar( of gold)?') -> |m| { if m.captures[0] == null { "pile" } else { "money" } }"#,
                result: Ok("pile of chocolate and money"),
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
        let count = arguments.optional("count");

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
                variables: vec![Variable {
                    kind: VariableKind::Exact(Kind::object(match_type)),
                }],
                output: Output::Kind(Kind::bytes()),
                example: example! {
                    title: "replace with hash",
                    source: r#"replace_with("received email from a@example.com", pattern: r'\w+@\w+\.\w+') -> |match| { sha2(match.string) }"#,
                    result: Ok(
                        "received email from 896bdca840c9304a5d0bdbeacc4ef359e3093f80c9777c9967e31ba0ff99ed58",
                    ),
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
    count: Option<Box<dyn Expression>>,
    closure: Closure,
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
        let count = self
            .count
            .map_resolve_with_default(ctx, || DEFAULT_COUNT.clone())?;
        let Closure {
            variables, block, ..
        } = &self.closure;

        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        replace_with(value, pattern, count, ctx, &runner)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}
