use regex::Regex;

use crate::compiler::prelude::*;

use super::util;
use std::sync::LazyLock;

static DEFAULT_NUMERIC_GROUPS: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The string to search.",
            default: None,
        },
        Parameter {
            keyword: "pattern",
            kind: kind::REGEX,
            required: true,
            description: "The regular expression pattern to search against.",
            default: None,
        },
        Parameter {
            keyword: "numeric_groups",
            kind: kind::BOOLEAN,
            required: false,
            description: "If `true`, the index of each group in the regular expression is also captured. Index `0`
contains the whole match.",
            default: Some(&DEFAULT_NUMERIC_GROUPS),
        },
    ]
});

fn parse_regex_all(value: &Value, numeric_groups: bool, pattern: &Regex) -> Resolved {
    let value = value.try_bytes_utf8_lossy()?;
    Ok(pattern
        .captures_iter(&value)
        .map(|capture| util::capture_regex_to_map(pattern, &capture, numeric_groups).into())
        .collect::<Vec<Value>>()
        .into())
}

#[derive(Clone, Copy, Debug)]
pub struct ParseRegexAll;

impl Function for ParseRegexAll {
    fn identifier(&self) -> &'static str {
        "parse_regex_all"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Parses the `value` using the provided [Regex](https://en.wikipedia.org/wiki/Regular_expression) `pattern`.

            This function differs from the `parse_regex` function in that it returns _all_ matches, not just the first.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a string.", "`pattern` is not a regex."]
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Matches return all capture groups corresponding to the leftmost matches in the text.",
            "Raises an error if no match is found.",
        ]
    }

    fn notices(&self) -> &'static [&'static str] {
        &[
            indoc! {"
                VRL aims to provide purpose-specific [parsing functions](/docs/reference/vrl/functions/#parse-functions)
                for common log formats. Before reaching for the `parse_regex` function, see if a VRL
                [`parse_*` function](/docs/reference/vrl/functions/#parse-functions) already exists
                for your format. If not, we recommend
                [opening an issue](https://github.com/vectordotdev/vector/issues/new?labels=type%3A+new+feature)
                to request support for the desired format.
            "},
            indoc! {"
                All values are returned as strings. We recommend manually coercing values to desired
                types as you see fit.
            "},
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let pattern = arguments.required("pattern");
        let numeric_groups = arguments.optional("numeric_groups");

        Ok(ParseRegexAllFn {
            value,
            pattern,
            numeric_groups,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse using Regex (all matches)",
                source: r#"parse_regex_all!("first group and second group.", r'(?P<number>\w+) group', numeric_groups: true)"#,
                result: Ok(indoc! { r#"[
               {"number": "first",
                "0": "first group",
                "1": "first"},
               {"number": "second",
                "0": "second group",
                "1": "second"}]"# }),
            },
            example! {
                title: "Parse using Regex (simple match)",
                source: r#"parse_regex_all!("apples and carrots, peaches and peas", r'(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)')"#,
                result: Ok(indoc! { r#"[
               {"fruit": "apples",
                "veg": "carrots"},
               {"fruit": "peaches",
                "veg": "peas"}]"# }),
            },
            example! {
                title: "Parse using Regex (all numeric groups)",
                source: r#"parse_regex_all!("apples and carrots, peaches and peas", r'(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)', numeric_groups: true)"#,
                result: Ok(indoc! { r#"[
               {"fruit": "apples",
                "veg": "carrots",
                "0": "apples and carrots",
                "1": "apples",
                "2": "carrots"},
               {"fruit": "peaches",
                "veg": "peas",
                "0": "peaches and peas",
                "1": "peaches",
                "2": "peas"}]"# }),
            },
            example! {
                title: "Parse using Regex with variables",
                source: r#"
                variable = r'(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)';
                parse_regex_all!("apples and carrots, peaches and peas", variable)"#,
                result: Ok(indoc! { r#"[
               {"fruit": "apples",
                "veg": "carrots"},
               {"fruit": "peaches",
                "veg": "peas"}]"# }),
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ParseRegexAllFn {
    value: Box<dyn Expression>,
    pattern: Box<dyn Expression>,
    numeric_groups: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseRegexAllFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let numeric_groups = self
            .numeric_groups
            .map_resolve_with_default(ctx, || DEFAULT_NUMERIC_GROUPS.clone())?;
        let pattern = self
            .pattern
            .resolve(ctx)?
            .as_regex()
            .ok_or_else(|| ExpressionError::from("failed to resolve regex"))?
            .clone();

        parse_regex_all(&value, numeric_groups.try_boolean()?, &pattern)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        if let Some(value) = self.pattern.resolve_constant(state)
            && let Some(regex) = value.as_regex()
        {
            return TypeDef::array(Collection::from_unknown(
                Kind::object(util::regex_kind(regex)).or_null(),
            ))
            .fallible();
        }

        TypeDef::array(Collection::from_unknown(
            Kind::object(Collection::from_unknown(Kind::bytes() | Kind::null())).or_null(),
        ))
        .fallible()
    }
}

#[cfg(test)]
#[allow(clippy::trivial_regex)]
mod tests {
    use crate::{btreemap, value};

    use super::*;

    test_function![
        parse_regex_all => ParseRegexAll;

        matches {
            args: func_args![
                value: "apples and carrots, peaches and peas",
                pattern: Regex::new(r"(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)").unwrap(),
            ],
            want: Ok(value!([{"fruit": "apples",
                              "veg": "carrots"},
                             {"fruit": "peaches",
                              "veg": "peas"}])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::null().or_object(btreemap! {
                    Field::from("fruit") => Kind::bytes(),
                    Field::from("veg") => Kind::bytes(),
                    Field::from("0") => Kind::bytes() | Kind::null(),
                    Field::from("1") => Kind::bytes() | Kind::null(),
                    Field::from("2") => Kind::bytes() | Kind::null(),
                }))).fallible(),
        }

        numeric_groups {
            args: func_args![
                value: "apples and carrots, peaches and peas",
                pattern: Regex::new(r"(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)").unwrap(),
                numeric_groups: true
            ],
            want: Ok(value!([{"fruit": "apples",
                              "veg": "carrots",
                              "0": "apples and carrots",
                              "1": "apples",
                              "2": "carrots"},
                             {"fruit": "peaches",
                              "veg": "peas",
                              "0": "peaches and peas",
                              "1": "peaches",
                              "2": "peas"}])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::null().or_object(btreemap! {
                    Field::from("fruit") => Kind::bytes(),
                    Field::from("veg") => Kind::bytes(),
                    Field::from("0") => Kind::bytes() | Kind::null(),
                    Field::from("1") => Kind::bytes() | Kind::null(),
                    Field::from("2") => Kind::bytes() | Kind::null(),
                }))).fallible(),
        }

        no_matches {
            args: func_args![
                value: "I don't match",
                pattern: Regex::new(r"(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)").unwrap()
            ],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::null().or_object(btreemap! {
                    Field::from("fruit") => Kind::bytes(),
                    Field::from("veg") => Kind::bytes(),
                    Field::from("0") => Kind::bytes() | Kind::null(),
                    Field::from("1") => Kind::bytes() | Kind::null(),
                    Field::from("2") => Kind::bytes() | Kind::null(),
                }))).fallible(),
        }
    ];
}
