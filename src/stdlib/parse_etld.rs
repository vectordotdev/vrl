use psl::Psl;
use publicsuffix::List;

use crate::compiler::prelude::*;
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy, Debug)]
pub struct ParseEtld;

impl Function for ParseEtld {
    fn identifier(&self) -> &'static str {
        "parse_etld"
    }

    fn usage(&self) -> &'static str {
        "Parses the [eTLD](https://developer.mozilla.org/en-US/docs/Glossary/eTLD) from `value` representing domain name."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
                description: "The domain string.",
            },
            Parameter {
                keyword: "plus_parts",
                kind: kind::INTEGER,
                required: false,
                description:
                    "Can be provided to get additional parts of the domain name. When 1 is passed,
eTLD+1 will be returned, which represents a domain registrable by a single
organization. Higher numbers will return subdomains.",
            },
            Parameter {
                keyword: "psl",
                kind: kind::BYTES,
                required: false,
                description: "Can be provided to use a different public suffix list.

By default, https://publicsuffix.org/list/public_suffix_list.dat is used.",
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse eTLD",
                source: r#"parse_etld!("sub.sussex.ac.uk")"#,
                result: Ok(indoc! {r#"
                {
                    "etld": "ac.uk",
                    "etld_plus": "ac.uk",
                    "known_suffix": true
                }
            "#}),
            },
            example! {
                title: "Parse eTLD+1",
                source: r#"parse_etld!("sub.sussex.ac.uk", plus_parts: 1)"#,
                result: Ok(indoc! {r#"
                {
                    "etld": "ac.uk",
                    "etld_plus": "sussex.ac.uk",
                    "known_suffix": true
                }
            "#}),
            },
            example! {
                title: "Parse eTLD with unknown suffix",
                source: r#"parse_etld!("vector.acmecorp")"#,
                result: Ok(indoc! {r#"
                {
                    "etld": "acmecorp",
                    "etld_plus": "acmecorp",
                    "known_suffix": false
                }
            "#}),
            },
            example! {
                title: "Parse eTLD with custom PSL",
                source: r#"parse_etld!("vector.acmecorp", psl: "lib/tests/tests/functions/custom_public_suffix_list.dat")"#,
                result: Ok(indoc! {r#"
                {
                    "etld": "acmecorp",
                    "etld_plus": "acmecorp",
                    "known_suffix": false
                }
            "#}),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let plus_parts = arguments.optional("plus_parts").unwrap_or_else(|| expr!(0));

        let psl_expr = arguments.optional_expr("psl");
        let mut psl: Option<List> = None;
        if let Some(psl_expr) = psl_expr {
            let psl_location = psl_expr
                .clone()
                .resolve_constant(state)
                .ok_or(function::Error::ExpectedStaticExpression {
                    keyword: "psl",
                    expr: psl_expr.clone(),
                })?
                .try_bytes_utf8_lossy()
                .map_err(|_| function::Error::InvalidArgument {
                    keyword: "psl",
                    value: format!("{psl_expr:?}").into(),
                    error: "psl should be a string",
                })?
                .into_owned();

            let path = Path::new(&psl_location);
            psl = Some(
                std::fs::read_to_string(path)
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "psl",
                        value: format!("{}", path.display()).into(),
                        error: "Unable to read psl file",
                    })?
                    .parse()
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "psl",
                        value: format!("{}", path.display()).into(),
                        error: "Unable to parse psl file",
                    })?,
            );
        }

        Ok(ParseEtldFn {
            value,
            plus_parts,
            psl,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseEtldFn {
    value: Box<dyn Expression>,
    plus_parts: Box<dyn Expression>,
    psl: Option<List>,
}

impl FunctionExpression for ParseEtldFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let plus_parts = match self.plus_parts.resolve(ctx)?.try_integer()? {
            x if x < 0 => 0,
            // TODO consider removal options
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            x => x as usize,
        };

        let suffix_result = if let Some(list) = &self.psl {
            list.suffix(string.as_bytes())
        } else {
            psl::suffix(string.as_bytes())
        };
        let etld = suffix_result.ok_or(format!("unable to determine eTLD for {string}"))?;
        let etld_string = core::str::from_utf8(etld.as_bytes())
            .map_err(|err| format!("could not convert eTLD to UTF8 {err}"))?;

        let etld_parts_count = etld_string.chars().filter(|c| *c == '.').count() + 1;
        let etld_plus_parts: Vec<&str> = string
            .rsplit('.')
            .take(etld_parts_count + plus_parts)
            .collect();

        let etld_plus = etld_plus_parts
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(".");

        let mut map = BTreeMap::<&str, Value>::new();

        map.insert("etld", etld_string.to_owned().into());
        map.insert("etld_plus", etld_plus.into());
        map.insert("known_suffix", etld.is_known().into());

        Ok(map
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<Value>())
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).fallible()
    }
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("etld".into(), Kind::bytes()),
        ("etld_plus".into(), Kind::bytes()),
        ("known_suffix".into(), Kind::boolean()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        parse_etld => ParseEtld;

        naive {
            args: func_args![value: value!("vector.dev")],
            want: Ok(value!({
                etld: "dev",
                etld_plus: "dev",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        naive_plus_one {
            args: func_args![value: value!("vector.dev"), plus_parts: 1],
            want: Ok(value!({
                etld: "dev",
                etld_plus: "vector.dev",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        psl {
            args: func_args![value: value!("sussex.ac.uk")],
            want: Ok(value!({
                etld: "ac.uk",
                etld_plus: "ac.uk",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        psl_plus_one {
            args: func_args![value: value!("sussex.ac.uk"), plus_parts: 1],
            want: Ok(value!({
                etld: "ac.uk",
                etld_plus: "sussex.ac.uk",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        short_plus {
            args: func_args![value: value!("vector.dev"), plus_parts: 10],
            want: Ok(value!({
                etld: "dev",
                etld_plus: "vector.dev",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        long_plus {
            args: func_args![value: value!("www.long.plus.test.vector.dev"), plus_parts: 4],
            want: Ok(value!({
                etld: "dev",
                etld_plus: "long.plus.test.vector.dev",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        unknown_tld {
            args: func_args![value: value!("vector.unknowndev")],
            want: Ok(value!({
                etld: "unknowndev",
                etld_plus: "unknowndev",
                known_suffix: false,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        utf8 {
            args: func_args![value: value!("www.食狮.中国")],
            want: Ok(value!({
                etld: "中国",
                etld_plus: "中国",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        utf8_plus_one {
            args: func_args![value: value!("www.食狮.中国"), plus_parts: 1],
            want: Ok(value!({
                etld: "中国",
                etld_plus: "食狮.中国",
                known_suffix: true,
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        empty_host {
            args: func_args![value: value!("")],
            want: Err("unable to determine eTLD for "),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        bad_psl_file {
            args: func_args![value: value!("vector.dev"), psl: value!("definitelynotafile")],
            want: Err("invalid argument"),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
