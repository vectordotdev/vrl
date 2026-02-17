use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_ALL: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::ARRAY,
            required: true,
            description: "The array.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "pattern",
            kind: kind::REGEX,
            required: true,
            description: "The regular expression pattern to match against.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "all",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether to match on all elements of `value`.",
            default: Some(&DEFAULT_ALL),
            enum_variants: None,
        },
    ]
});

fn match_array(list: Value, pattern: Value, all: Value) -> Resolved {
    let pattern = pattern.try_regex()?;
    let list = list.try_array()?;
    let all = all.try_boolean()?;
    let matcher = |i: &Value| match i.try_bytes_utf8_lossy() {
        Ok(v) => pattern.is_match(&v),
        _ => false,
    };
    let included = if all {
        list.iter().all(matcher)
    } else {
        list.iter().any(matcher)
    };
    Ok(included.into())
}

#[derive(Clone, Copy, Debug)]
pub struct MatchArray;

impl Function for MatchArray {
    fn identifier(&self) -> &'static str {
        "match_array"
    }

    fn usage(&self) -> &'static str {
        "Determines whether the elements in the `value` array matches the `pattern`. By default, it checks that at least one element matches, but can be set to determine if all the elements match."
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Match at least one element",
                source: r#"match_array(["foobar", "bazqux"], r'foo')"#,
                result: Ok("true"),
            },
            example! {
                title: "Match all elements",
                source: r#"match_array(["foo", "foobar", "barfoo"], r'foo', all: true)"#,
                result: Ok("true"),
            },
            example! {
                title: "No matches",
                source: r#"match_array(["bazqux", "xyz"], r'foo')"#,
                result: Ok("false"),
            },
            example! {
                title: "Not all elements match",
                source: r#"match_array(["foo", "foobar", "baz"], r'foo', all: true)"#,
                result: Ok("false"),
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
        let all = arguments.optional("all");

        Ok(MatchArrayFn {
            value,
            pattern,
            all,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MatchArrayFn {
    value: Box<dyn Expression>,
    pattern: Box<dyn Expression>,
    all: Option<Box<dyn Expression>>,
}

impl FunctionExpression for MatchArrayFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let list = self.value.resolve(ctx)?;
        let pattern = self.pattern.resolve(ctx)?;
        let all = self
            .all
            .map_resolve_with_default(ctx, || DEFAULT_ALL.clone())?;

        match_array(list, pattern, all)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
#[allow(clippy::trivial_regex)]
mod tests {
    use super::*;
    use crate::value;
    use regex::Regex;

    test_function![
        match_array => MatchArray;

        default {
            args: func_args![
                value: value!(["foo", "foobar", "barfoo"]),
                pattern: Value::Regex(Regex::new("foo").unwrap().into())
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        all {
            args: func_args![
                value: value!(["foo", "foobar", "barfoo"]),
                pattern: Value::Regex(Regex::new("foo").unwrap().into()),
                all: value!(true),
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        not_all {
            args: func_args![
                value: value!(["foo", "foobar", "baz"]),
                pattern: Value::Regex(Regex::new("foo").unwrap().into()),
                all: value!(true),
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        mixed_values {
            args: func_args![
                value: value!(["foo", "123abc", 1, true, [1,2,3]]),
                pattern: Value::Regex(Regex::new("abc").unwrap().into())
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        mixed_values_no_match {
            args: func_args![
                value: value!(["foo", "123abc", 1, true, [1,2,3]]),
                pattern: Value::Regex(Regex::new("xyz").unwrap().into()),
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        mixed_values_no_match_all {
            args: func_args![
                value: value!(["foo", "123abc", 1, true, [1,2,3]]),
                pattern: Value::Regex(Regex::new("abc`").unwrap().into()),
                all: value!(true),
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
