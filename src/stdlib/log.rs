use crate::compiler::function::EnumVariant;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_LEVEL: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("info")));
static DEFAULT_RATE_LIMIT_SECS: LazyLock<Value> = LazyLock::new(|| Value::Integer(1));

static LEVEL_ENUM: &[EnumVariant] = &[
    EnumVariant {
        value: "trace",
        description: "Log at the `trace` level.",
    },
    EnumVariant {
        value: "debug",
        description: "Log at the `debug` level.",
    },
    EnumVariant {
        value: "info",
        description: "Log at the `info` level.",
    },
    EnumVariant {
        value: "warn",
        description: "Log at the `warn` level.",
    },
    EnumVariant {
        value: "error",
        description: "Log at the `error` level.",
    },
];

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required("value", kind::ANY, "The value to log."),
        Parameter::optional("level", kind::BYTES, "The log level.")
            .default(&DEFAULT_LEVEL)
            .enum_variants(LEVEL_ENUM),
        Parameter::optional("rate_limit_secs", kind::INTEGER, "Specifies that the log message is output no more than once per the given number of seconds.
Use a value of `0` to turn rate limiting off.")
            .default(&DEFAULT_RATE_LIMIT_SECS),
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct Log;

impl Function for Log {
    fn identifier(&self) -> &'static str {
        "log"
    }

    fn usage(&self) -> &'static str {
        "Logs the `value` to [stdout](https://en.wikipedia.org/wiki/Standard_streams#Standard_output_(stdout)) at the specified `level`."
    }

    fn category(&self) -> &'static str {
        Category::Debug.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::NULL
    }

    fn pure(&self) -> bool {
        false
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Log a message",
                source: r#"log("Hello, World!", level: "info", rate_limit_secs: 60)"#,
                result: Ok("null"),
            },
            example! {
                title: "Log an error",
                source: indoc! {r#"
                    . = { "field": "not an integer" }
                    _, err = to_int(.field)
                    if err != null {
                        log(err, level: "error")
                    }
                "#},
                result: Ok("null"),
            },
        ]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let levels = vec![
            "trace".into(),
            "debug".into(),
            "info".into(),
            "warn".into(),
            "error".into(),
        ];

        let value = arguments.required("value");
        let level = arguments
            .optional_enum("level", &levels, state)?
            .unwrap_or_else(|| DEFAULT_LEVEL.clone())
            .try_bytes()
            .expect("log level not bytes");
        let rate_limit_secs = arguments.optional("rate_limit_secs");

        Ok(implementation::LogFn {
            span: ctx.span(),
            value,
            level,
            rate_limit_secs,
        }
        .as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(ctx.span(), TypeDef::null().infallible()).as_expr())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod implementation {
    use tracing::{debug, error, info, trace, warn};

    use super::DEFAULT_RATE_LIMIT_SECS;
    use crate::compiler::prelude::*;

    pub(super) fn log(
        rate_limit_secs: Value,
        level: &Bytes,
        value: &Value,
        span: Span,
    ) -> Resolved {
        let rate_limit_secs = rate_limit_secs.try_integer()?;
        let res = value.to_string_lossy();
        match level.as_ref() {
            b"trace" => {
                trace!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start());
            }
            b"debug" => {
                debug!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start());
            }
            b"warn" => {
                warn!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start());
            }
            b"error" => {
                error!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start());
            }
            _ => {
                info!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start());
            }
        }
        Ok(Value::Null)
    }

    #[derive(Debug, Clone)]
    pub(super) struct LogFn {
        pub(super) span: Span,
        pub(super) value: Box<dyn Expression>,
        pub(super) level: Bytes,
        pub(super) rate_limit_secs: Option<Box<dyn Expression>>,
    }

    impl FunctionExpression for LogFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let rate_limit_secs = self
                .rate_limit_secs
                .map_resolve_with_default(ctx, || DEFAULT_RATE_LIMIT_SECS.clone())?;

            let span = self.span;

            log(rate_limit_secs, &self.level, &value, span)
        }

        fn type_def(&self, _: &state::TypeState) -> TypeDef {
            TypeDef::null().infallible().impure()
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use tracing_test::traced_test;

    use super::*;
    use crate::value;

    test_function![
        log => Log;

        doesnotcrash {
            args: func_args! [ value: value!(42),
                               level: value!("warn"),
                               rate_limit_secs: value!(5) ],
            want: Ok(Value::Null),
            tdef: TypeDef::null().infallible().impure(),
        }
    ];

    #[traced_test]
    #[test]
    fn output_quotes() {
        // Check that a message is logged without additional quotes
        implementation::log(
            value!(1),
            &Bytes::from("warn"),
            &value!("simple test message"),
            Span::default(),
        )
        .unwrap();

        assert!(!logs_contain("\"simple test message\""));
        assert!(logs_contain("simple test message"));
    }
}
