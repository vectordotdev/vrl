use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use crate::compiler::prelude::*;
    use crate::datadog_grok::{parse_grok, parse_grok_rules::GrokRule};
    use crate::diagnostic::{Label, Span};
    use std::fmt;

    #[derive(Debug)]
    pub(crate) enum Error {
        InvalidGrokPattern(crate::datadog_grok::parse_grok_rules::Error),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Error::InvalidGrokPattern(err) => err.fmt(f),
            }
        }
    }

    impl std::error::Error for Error {}

    impl DiagnosticMessage for Error {
        fn code(&self) -> usize {
            109
        }

        fn labels(&self) -> Vec<Label> {
            match self {
                Error::InvalidGrokPattern(err) => {
                    vec![Label::primary(
                        format!("grok pattern error: {err}"),
                        Span::default(),
                    )]
                }
            }
        }
    }

    #[derive(Clone, Debug)]
    pub(super) struct ParseGroksFn {
        pub(super) value: Box<dyn Expression>,
        pub(super) grok_rules: Vec<GrokRule>,
    }

    impl FunctionExpression for ParseGroksFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let bytes = value.try_bytes_utf8_lossy()?;

            let v = parse_grok::parse_grok(bytes.as_ref(), &self.grok_rules)
                .map_err(|err| format!("unable to parse grok: {err}"))?;

            Ok(v)
        }

        fn type_def(&self, _: &state::TypeState) -> TypeDef {
            TypeDef::object(Collection::any()).fallible()
        }
    }
}

#[allow(clippy::wildcard_imports)]
#[cfg(not(target_arch = "wasm32"))]
use non_wasm::*;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs::File, io::BufReader, path::Path};

#[derive(Clone, Copy, Debug)]
pub struct ParseGroks;

impl Function for ParseGroks {
    fn identifier(&self) -> &'static str {
        "parse_groks"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "patterns",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "aliases",
                kind: kind::OBJECT,
                required: false,
            },
            Parameter {
                keyword: "alias_sources",
                kind: kind::ARRAY,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse grok pattern",
            source: indoc! {r#"
                parse_groks!(
                    "2020-10-02T23:22:12.223222Z info hello world",
                    patterns: [
                        "%{common_prefix} %{_status} %{_message}",
                        "%{common_prefix} %{_message}"
                    ],
                    aliases: {
                        "common_prefix": "%{_timestamp} %{_loglevel}",
                        "_timestamp": "%{TIMESTAMP_ISO8601:timestamp}",
                        "_loglevel": "%{LOGLEVEL:level}",
                        "_status": "%{POSINT:status}",
                        "_message": "%{GREEDYDATA:message}"
                    })
            "#},
            result: Ok(indoc! {r#"
                {
                    "timestamp": "2020-10-02T23:22:12.223222Z",
                    "level": "info",
                    "message": "hello world"
                }
            "#}),
        }]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        use std::collections::BTreeMap;

        let value = arguments.required("value");

        let patterns = arguments
            .required_array("patterns")?
            .into_iter()
            .map(|expr| {
                let pattern = expr
                    .clone()
                    .resolve_constant(state)
                    .ok_or(function::Error::ExpectedStaticExpression {
                        keyword: "patterns",
                        expr: expr.clone(),
                    })?
                    .try_bytes_utf8_lossy()
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "patterns",
                        value: format!("{expr:?}").into(),
                        error: "grok pattern should be a string",
                    })?
                    .into_owned();
                Ok(pattern)
            })
            .collect::<std::result::Result<Vec<String>, function::Error>>()?;

        let mut aliases = arguments
            .optional_object("aliases")?
            .unwrap_or_default()
            .into_iter()
            .map(|(key, expr)| {
                let alias = expr
                    .clone()
                    .resolve_constant(state)
                    .ok_or(function::Error::ExpectedStaticExpression {
                        keyword: "aliases",
                        expr: expr.clone(),
                    })?
                    .try_bytes_utf8_lossy()
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "aliases",
                        value: format!("{expr:?}").into(),
                        error: "alias pattern should be a string",
                    })?
                    .into_owned();
                Ok((key, alias))
            })
            .collect::<std::result::Result<BTreeMap<KeyString, String>, function::Error>>()?;

        let alias_sources = arguments
            .optional_array("alias_sources")?
            .unwrap_or_default()
            .into_iter()
            .map(|expr| {
                let path = expr
                    .clone()
                    .resolve_constant(state)
                    .ok_or(function::Error::ExpectedStaticExpression {
                        keyword: "alias_sources",
                        expr: expr.clone(),
                    })?
                    .try_bytes_utf8_lossy()
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "alias_sources",
                        value: format!("{expr:?}").into(),
                        error: "alias source should be a string",
                    })?
                    .into_owned();
                Ok(path)
            })
            .collect::<std::result::Result<Vec<String>, function::Error>>()?;

        for src in alias_sources {
            let path = Path::new(&src);
            let file = File::open(path).map_err(|_| function::Error::InvalidArgument {
                keyword: "alias_sources",
                value: format!("{path:?}").into(),
                error: "Unable to open alias source file",
            })?;
            let reader = BufReader::new(file);
            let mut src_aliases =
                serde_json::from_reader(reader).map_err(|_| function::Error::InvalidArgument {
                    keyword: "alias_sources",
                    value: format!("{path:?}").into(),
                    error: "Unable to read alias source",
                })?;

            aliases.append(&mut src_aliases);
        }

        // we use a datadog library here because it is a superset of grok
        let grok_rules = crate::datadog_grok::parse_grok_rules::parse_grok_rules(
            &patterns, aliases,
        )
        .map_err(|e| Box::new(Error::InvalidGrokPattern(e)) as Box<dyn DiagnosticMessage>)?;

        Ok(ParseGroksFn { value, grok_rules }.as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(
            ctx.span(),
            TypeDef::object(Collection::any()).fallible(),
        )
        .as_expr())
    }
}

#[cfg(test)]
mod test {
    use crate::btreemap;
    use crate::value;
    use crate::value::Value;

    use super::*;

    test_function![
        parse_grok => ParseGroks;

        invalid_grok {
            args: func_args![ value: "foo",
                              patterns: vec!["%{NOG}"]],
            want: Err("failed to parse grok expression '(?m)\\A%{NOG}\\z': The given pattern definition name \"NOG\" could not be found in the definition map"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        error {
            args: func_args![ value: "an ungrokkable message",
                              patterns: vec!["%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"]],
            want: Err("unable to parse grok: value does not match any rule"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        error2 {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z an ungrokkable message",
                              patterns: vec!["%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"]],
            want: Err("unable to parse grok: value does not match any rule"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        error3 {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z info Hello world",
                              patterns: vec!["%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"],
                              aliases: value!({
                                  "TEST": 3
                              })],
            want: Err("invalid argument"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        parsed {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z info Hello world",
                              patterns: vec!["%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"]],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "message" => "Hello world",
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        parsed2 {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z",
                              patterns: vec!["(%{TIMESTAMP_ISO8601:timestamp}|%{LOGLEVEL:level})"]],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        multiple_patterns_and_aliases_first_pattern_matches {
            args: func_args![
                value: "2020-10-02T23:22:12.223222Z info 200 hello world",
                patterns: Value::Array(vec![
                    "%{common_prefix} %{_status} %{_message}".into(),
                    "%{common_prefix} %{_message}".into(),
                    ]),
                aliases: value!({
                    "common_prefix": "%{_timestamp} %{_loglevel}",
                    "_timestamp": "%{TIMESTAMP_ISO8601:timestamp}",
                    "_loglevel": "%{LOGLEVEL:level}",
                    "_status": "%{POSINT:status}",
                    "_message": "%{GREEDYDATA:message}"
                })
            ],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "status" => "200",
                "message" => "hello world"
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        presence_of_alias_sources_argument {
            args: func_args![
                value: "2020-10-02T23:22:12.223222Z info 200 hello world",
                patterns: Value::Array(vec![
                    "%{common_prefix} %{_status} %{_message}".into(),
                    "%{common_prefix} %{_message}".into(),
                    ]),
                aliases: value!({
                    "common_prefix": "%{_timestamp} %{_loglevel}",
                    "_timestamp": "%{TIMESTAMP_ISO8601:timestamp}",
                    "_loglevel": "%{LOGLEVEL:level}",
                    "_status": "%{POSINT:status}",
                    "_message": "%{GREEDYDATA:message}"
                }),
                alias_sources: Value::Array(vec![]),
            ],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "status" => "200",
                "message" => "hello world"
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        multiple_patterns_and_aliases_second_pattern_matches {
            args: func_args![
                value: "2020-10-02T23:22:12.223222Z info hello world",
                patterns: Value::Array(vec![
                    "%{common_prefix} %{_status} %{_message}".into(),
                    "%{common_prefix} %{_message}".into(),
                    ]),
                aliases: value!({
                    "common_prefix": "%{_timestamp} %{_loglevel}",
                    "_timestamp": "%{TIMESTAMP_ISO8601:timestamp}",
                    "_loglevel": "%{LOGLEVEL:level}",
                    "_status": "%{POSINT:status}",
                    "_message": "%{GREEDYDATA:message}"
                })
            ],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "message" => "hello world"
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        datadog_nginx {
            args: func_args![
                value: r#"127.0.0.1 - frank [13/Jul/2016:10:55:36] "GET /apache_pb.gif HTTP/1.0" 200 2326 0.202 "http://www.perdu.com/" "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36" "-""#,
                patterns: Value::Array(vec![
                    "%{access_common}".into(),
                    r#"%{access_common} (%{number:duration:scale(1000000000)} )?"%{_referer}" "%{_user_agent}"( "%{_x_forwarded_for}")?.*"#.into(),
                    ]),
                aliases: value!({
                    "access_common": r#"%{_client_ip} %{_ident} %{_auth} \[%{_date_access}\] "(?>%{_method} |)%{_url}(?> %{_version}|)" %{_status_code} (?>%{_bytes_written}|-)"#,
                    "_auth": r#"%{notSpace:http.auth:nullIf("-")}"#,
                    "_bytes_written": "%{integer:network.bytes_written}",
                    "_client_ip": "%{ipOrHost:network.client.ip}",
                    "_version": r#"HTTP\/%{regex("\\d+\\.\\d+"):http.version}"#,
                    "_url": "%{notSpace:http.url}",
                    "_ident": "%{notSpace:http.ident}",
                    "_user_agent": r#"%{regex("[^\\\"]*"):http.useragent}"#,
                    "_referer": "%{notSpace:http.referer}",
                    "_status_code": "%{integer:http.status_code}",
                    "_method": "%{word:http.method}",
                    "_date_access": "%{notSpace:date_access}",
                    "_x_forwarded_for": r#"%{regex("[^\\\"]*"):http._x_forwarded_for:nullIf("-")}"#
                })
            ],
            want: Ok(Value::Object(btreemap! {
                "date_access" => "13/Jul/2016:10:55:36",
                "duration" => 202_000_000,
                "http" => btreemap! {
                    "auth" => "frank",
                    "ident" => "-",
                    "method" => "GET",
                    "status_code" => 200,
                    "url" => "/apache_pb.gif",
                    "version" => "1.0",
                    "referer" => "http://www.perdu.com/",
                    "useragent" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36",
                },
                "network" => btreemap! {
                    "bytes_written" => 2326,
                    "client" => btreemap! {
                        "ip" => "127.0.0.1"
                    }
                }
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }
    ];
}
