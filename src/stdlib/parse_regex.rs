use super::util;
use crate::compiler::prelude::*;
use crate::value::KeyString;
use regex::Regex;

static DEFAULT_NUMERIC_GROUPS: Value = Value::Boolean(false);

const PARAMETERS: &[Parameter] = &[
    Parameter::required("value", kind::BYTES, "The string to search."),
    Parameter::required(
        "pattern",
        kind::REGEX,
        "The regular expression pattern to search against.",
    ),
    Parameter::optional(
        "numeric_groups",
        kind::BOOLEAN,
        "If true, the index of each group in the regular expression is also captured. Index `0`
contains the whole match.",
    )
    .default(&DEFAULT_NUMERIC_GROUPS),
];

fn parse_regex(
    value: &Value,
    pattern: &Regex,
    capture_info: &[(KeyString, usize)],
    numeric_groups: bool,
) -> Resolved {
    let value = value.try_bytes_utf8_lossy()?;
    let parsed = pattern
        .captures(&value)
        .map(|capture| util::capture_regex_to_map(&capture, capture_info, numeric_groups))
        .ok_or("could not find any pattern matches")?;
    Ok(parsed.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ParseRegex;

impl Function for ParseRegex {
    fn identifier(&self) -> &'static str {
        "parse_regex"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Parses the `value` using the provided [Regex](https://en.wikipedia.org/wiki/Regular_expression) `pattern`.

            This function differs from the `parse_regex_all` function in that it returns only the first match.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` fails to parse using the provided `pattern`."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
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
            indoc! {"
                When `pattern` is a dynamic expression (e.g. a variable or the result of `to_regex`),
                the regex is compiled on every function call. For high-throughput pipelines, prefer
                a regex literal so the pattern is compiled once at program compile time.
            "},
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
        let pattern = arguments.required("pattern");
        let capture_info = pattern.resolve_constant(state).and_then(|v| {
            v.as_regex().map(|r| {
                r.capture_names()
                    .enumerate()
                    .filter_map(|(i, name)| name.map(|n| (KeyString::from(n), i)))
                    .collect::<Vec<_>>()
            })
        });
        let numeric_groups = arguments.optional("numeric_groups");

        Ok(ParseRegexFn {
            value,
            pattern,
            capture_info,
            numeric_groups,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse using Regex (with capture groups)",
                source: r#"parse_regex!("first group and second group.", r'(?P<number>.*?) group')"#,
                result: Ok(r#"{"number": "first"}"#),
            },
            example! {
                title: "Parse using Regex (without capture groups)",
                source: r#"parse_regex!("first group and second group.", r'(\w+) group', numeric_groups: true)"#,
                result: Ok(indoc! { r#"{
                "0": "first group",
                "1": "first"
            }"# }),
            },
            example! {
                title: "Parse using Regex with simple match",
                source: r#"parse_regex!("8.7.6.5 - zorp", r'^(?P<host>[\w\.]+) - (?P<user>[\w]+)')"#,
                result: Ok(indoc! { r#"{
                "host": "8.7.6.5",
                "user": "zorp"
            }"# }),
            },
            example! {
                title: "Parse using Regex with all numeric groups",
                source: r#"parse_regex!("8.7.6.5 - zorp", r'^(?P<host>[\w\.]+) - (?P<user>[\w]+)', numeric_groups: true)"#,
                result: Ok(indoc! { r#"{
                "0": "8.7.6.5 - zorp",
                "1": "8.7.6.5",
                "2": "zorp",
                "host": "8.7.6.5",
                "user": "zorp"
            }"# }),
            },
            example! {
                title: "Parse using Regex with variables",
                source: indoc! {r#"
                    variable = r'^(?P<host>[\w\.]+) - (?P<user>[\w]+)';
                    parse_regex!("8.7.6.5 - zorp", variable)
                "#},
                result: Ok(indoc! { r#"{
                "host": "8.7.6.5",
                "user": "zorp"
            }"# }),
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ParseRegexFn {
    value: Box<dyn Expression>,
    pattern: Box<dyn Expression>,
    capture_info: Option<Vec<(KeyString, usize)>>,
    numeric_groups: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseRegexFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let numeric_groups = self
            .numeric_groups
            .map_resolve_with_default(ctx, || DEFAULT_NUMERIC_GROUPS.clone())?
            .try_boolean()?;
        let resolved = self.pattern.resolve(ctx)?;
        let pattern = resolved
            .as_regex()
            .ok_or_else(|| ExpressionError::from("failed to resolve regex"))?;

        let dynamic_capture_info;
        let capture_info: &[(KeyString, usize)] = if let Some(info) = &self.capture_info {
            info.as_slice()
        } else {
            dynamic_capture_info = pattern
                .capture_names()
                .enumerate()
                .filter_map(|(i, name)| name.map(|n| (KeyString::from(n), i)))
                .collect::<Vec<_>>();
            dynamic_capture_info.as_slice()
        };
        parse_regex(&value, pattern, capture_info, numeric_groups)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        if let Some(value) = self.pattern.resolve_constant(state)
            && let Some(regex) = value.as_regex()
        {
            return TypeDef::object(util::regex_kind(regex)).fallible();
        }

        TypeDef::object(Collection::from_unknown(Kind::bytes() | Kind::null())).fallible()
    }
}

#[cfg(test)]
#[allow(clippy::trivial_regex)]
mod tests {
    use super::*;
    use crate::{btreemap, value};

    test_function![
        find => ParseRegex;

        numeric_groups {
            args: func_args! [
                value: "5.86.210.12 - zieme4647 5667 [19/06/2019:17:20:49 -0400] \"GET /embrace/supply-chains/dynamic/vertical\" 201 20574",
                pattern: Regex::new(r#"^(?P<host>[\w\.]+) - (?P<user>[\w]+) (?P<bytes_in>[\d]+) \[(?P<timestamp>.*)\] "(?P<method>[\w]+) (?P<path>.*)" (?P<status>[\d]+) (?P<bytes_out>[\d]+)$"#)
                    .unwrap(),
                numeric_groups: true,
            ],
            want: Ok(value!({"bytes_in": "5667",
                             "host": "5.86.210.12",
                             "user": "zieme4647",
                             "timestamp": "19/06/2019:17:20:49 -0400",
                             "method": "GET",
                             "path": "/embrace/supply-chains/dynamic/vertical",
                             "status": "201",
                             "bytes_out": "20574",
                             "0": "5.86.210.12 - zieme4647 5667 [19/06/2019:17:20:49 -0400] \"GET /embrace/supply-chains/dynamic/vertical\" 201 20574",
                             "1": "5.86.210.12",
                             "2": "zieme4647",
                             "3": "5667",
                             "4": "19/06/2019:17:20:49 -0400",
                             "5": "GET",
                             "6": "/embrace/supply-chains/dynamic/vertical",
                             "7": "201",
                             "8": "20574",
            })),
            tdef: TypeDef::object(btreemap! {
                    Field::from("bytes_in") => Kind::bytes(),
                    Field::from("host") => Kind::bytes(),
                    Field::from("user") => Kind::bytes(),
                    Field::from("timestamp") => Kind::bytes(),
                    Field::from("method") => Kind::bytes(),
                    Field::from("path") => Kind::bytes(),
                    Field::from("status") => Kind::bytes(),
                    Field::from("bytes_out") => Kind::bytes(),
                    Field::from("0") => Kind::bytes() | Kind::null(),
                    Field::from("1") => Kind::bytes() | Kind::null(),
                    Field::from("2") => Kind::bytes() | Kind::null(),
                    Field::from("3") => Kind::bytes() | Kind::null(),
                    Field::from("4") => Kind::bytes() | Kind::null(),
                    Field::from("5") => Kind::bytes() | Kind::null(),
                    Field::from("6") => Kind::bytes() | Kind::null(),
                    Field::from("7") => Kind::bytes() | Kind::null(),
                    Field::from("8") => Kind::bytes() | Kind::null(),
                }).fallible(),
        }

        single_match {
            args: func_args! [
                value: "first group and second group",
                pattern: Regex::new("(?P<number>.*?) group").unwrap()
            ],
            want: Ok(value!({"number": "first"})),
            tdef: TypeDef::object(btreemap! {
                        Field::from("number") => Kind::bytes(),
                        Field::from("0") => Kind::bytes() | Kind::null(),
                        Field::from("1") => Kind::bytes() | Kind::null(),
                }).fallible(),
        }

        no_match {
            args: func_args! [
                value: "I don't match",
                pattern: Regex::new(r#"^(?P<host>[\w\.]+) - (?P<user>[\w]+) (?P<bytes_in>[\d]+) \[(?P<timestamp>.*)\] "(?P<method>[\w]+) (?P<path>.*)" (?P<status>[\d]+) (?P<bytes_out>[\d]+)$"#)
                            .unwrap()
            ],
            want: Err("could not find any pattern matches"),
            tdef: TypeDef::object(btreemap! {
                    Field::from("host") => Kind::bytes(),
                    Field::from("user") => Kind::bytes(),
                    Field::from("bytes_in") => Kind::bytes(),
                    Field::from("timestamp") => Kind::bytes(),
                    Field::from("method") => Kind::bytes(),
                    Field::from("path") => Kind::bytes(),
                    Field::from("status") => Kind::bytes(),
                    Field::from("bytes_out") => Kind::bytes(),
                    Field::from("0") => Kind::bytes() | Kind::null(),
                    Field::from("1") => Kind::bytes() | Kind::null(),
                    Field::from("2") => Kind::bytes() | Kind::null(),
                    Field::from("3") => Kind::bytes() | Kind::null(),
                    Field::from("4") => Kind::bytes() | Kind::null(),
                    Field::from("5") => Kind::bytes() | Kind::null(),
                    Field::from("6") => Kind::bytes() | Kind::null(),
                    Field::from("7") => Kind::bytes() | Kind::null(),
                    Field::from("8") => Kind::bytes() | Kind::null(),
                }).fallible(),
        }
    ];
}
