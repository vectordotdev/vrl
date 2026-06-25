use regex::Regex;

use super::util;
use crate::compiler::prelude::*;
use crate::stdlib::util::RegexWithCaptureInfo;

static DEFAULT_NUMERIC_GROUPS: Value = Value::Boolean(false);

const PARAMETERS: &[Parameter] = &[
    Parameter::required("value", kind::ANY, "The string to search."),
    Parameter::required(
        "pattern",
        kind::REGEX,
        "The regular expression pattern to search against.",
    ),
    Parameter::optional(
        "numeric_groups",
        kind::BOOLEAN,
        "If `true`, the index of each group in the regular expression is also captured. Index `0`
contains the whole match.",
    )
    .default(&DEFAULT_NUMERIC_GROUPS),
];

fn parse_regex_all(
    value: &Value,
    pattern: &Regex,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
) -> Resolved {
    let value = value.try_bytes_utf8_lossy()?;
    Ok(pattern
        .captures_iter(&value)
        .map(|capture| util::capture_regex_to_map(&capture, capture_info, numeric_groups).into())
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
            util::DYNAMIC_REGEX_NOTICE,
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let pattern = match arguments.required_regex("pattern", state) {
            ConstOrExpr::Const(r) => ConstOrExpr::Const(RegexWithCaptureInfo::new(r)),
            ConstOrExpr::Expr(e) => ConstOrExpr::Expr(e),
        };
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
                source: indoc! {r#"
                    variable = r'(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)';
                    parse_regex_all!("apples and carrots, peaches and peas", variable)
                "#},
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
    pattern: ConstOrExpr<RegexWithCaptureInfo>,
    numeric_groups: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseRegexAllFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let numeric_groups = self
            .numeric_groups
            .map_resolve_with_default(ctx, || DEFAULT_NUMERIC_GROUPS.clone())?
            .try_boolean()?;

        match &self.pattern {
            ConstOrExpr::Const(pattern) => parse_regex_all(
                &value,
                &pattern.regex,
                &pattern.capture_info,
                numeric_groups,
            ),
            ConstOrExpr::Expr(expr) => {
                let resolved = expr.resolve(ctx)?;
                let pattern = resolved
                    .as_regex()
                    .ok_or_else(|| ExpressionError::from("failed to resolve regex"))?;
                let dynamic_capture_info = util::build_capture_info(pattern);
                parse_regex_all(&value, pattern, &dynamic_capture_info, numeric_groups)
            }
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        match &self.pattern {
            ConstOrExpr::Const(pattern) => TypeDef::array(Collection::from_unknown(
                Kind::object(util::regex_kind(&pattern.regex)).or_null(),
            ))
            .fallible(),
            ConstOrExpr::Expr(_) => TypeDef::array(Collection::from_unknown(
                Kind::object(Collection::from_unknown(Kind::bytes() | Kind::null())).or_null(),
            ))
            .fallible(),
        }
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
