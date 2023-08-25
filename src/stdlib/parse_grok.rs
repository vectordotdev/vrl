use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use crate::compiler::prelude::*;
    use crate::diagnostic::{Label, Span};
    use crate::value::Value;
    pub(super) use std::sync::Arc;
    use std::{collections::BTreeMap, fmt};

    fn parse_grok(value: Value, pattern: Arc<grok::Pattern>) -> Resolved {
        let bytes = value.try_bytes_utf8_lossy()?;
        match pattern.match_against(&bytes) {
            Some(matches) => {
                let mut result = BTreeMap::new();

                for (name, value) in &matches {
                    result.insert(name.to_string(), Value::from(value));
                }

                Ok(Value::from(result))
            }
            None => Err("unable to parse input with grok pattern".into()),
        }
    }

    #[derive(Debug)]
    pub(crate) enum Error {
        InvalidGrokPattern(grok::Error),
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
    pub(super) struct ParseGrokFn {
        pub(super) value: Box<dyn Expression>,

        // Wrapping pattern in an Arc, as cloning the pattern could otherwise be expensive.
        pub(super) pattern: Arc<grok::Pattern>,
    }

    impl FunctionExpression for ParseGrokFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let pattern = self.pattern.clone();

            parse_grok(value, pattern)
        }

        fn type_def(&self, _: &TypeState) -> TypeDef {
            TypeDef::object(Collection::any()).fallible()
        }
    }
}

#[allow(clippy::wildcard_imports)]
#[cfg(not(target_arch = "wasm32"))]
use non_wasm::*;

#[derive(Clone, Copy, Debug)]
pub struct ParseGrok;

impl Function for ParseGrok {
    fn identifier(&self) -> &'static str {
        "parse_grok"
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
                kind: kind::BYTES,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse grok pattern",
            source: indoc! {r#"
                value = "2020-10-02T23:22:12.223222Z info Hello world"
                pattern = "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"

                parse_grok!(value, pattern)
            "#},
            result: Ok(indoc! {r#"
                {
                    "timestamp": "2020-10-02T23:22:12.223222Z",
                    "level": "info",
                    "message": "Hello world"
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
        let value = arguments.required("value");

        let pattern = arguments
            .required_literal("pattern", state)?
            .try_bytes_utf8_lossy()
            .expect("grok pattern not bytes")
            .into_owned();

        let mut grok = grok::Grok::with_default_patterns();
        let pattern =
            Arc::new(grok.compile(&pattern, true).map_err(|e| {
                Box::new(Error::InvalidGrokPattern(e)) as Box<dyn DiagnosticMessage>
            })?);

        Ok(ParseGrokFn { value, pattern }.as_expr())
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
    use crate::value::Value;

    use super::*;

    test_function![
        parse_grok => ParseGrok;

        invalid_grok {
            args: func_args![ value: "foo",
                              pattern: "%{NOG}"],
            want: Err("The given pattern definition name \"NOG\" could not be found in the definition map"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        error {
            args: func_args![ value: "an ungrokkable message",
                              pattern: "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"],
            want: Err("unable to parse input with grok pattern"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        error2 {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z an ungrokkable message",
                              pattern: "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"],
            want: Err("unable to parse input with grok pattern"),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        parsed {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z info Hello world",
                              pattern: "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "message" => "Hello world",
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }

        parsed2 {
            args: func_args![ value: "2020-10-02T23:22:12.223222Z",
                              pattern: "(%{TIMESTAMP_ISO8601:timestamp}|%{LOGLEVEL:level})"],
            want: Ok(Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
            })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }
    ];
}
