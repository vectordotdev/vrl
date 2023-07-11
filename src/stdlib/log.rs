use crate::compiler::prelude::*;
use crate::value;
use tracing::{debug, error, info, trace, warn};

fn log(rate_limit_secs: Value, level: &Bytes, value: Value, span: Span) -> Resolved {
    let rate_limit_secs = rate_limit_secs.try_integer()?;
    let res = value.to_string_lossy();
    match level.as_ref() {
        b"trace" => {
            trace!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start())
        }
        b"debug" => {
            debug!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start())
        }
        b"warn" => {
            warn!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start())
        }
        b"error" => {
            error!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start())
        }
        _ => {
            info!(message = %res, internal_log_rate_secs = rate_limit_secs, vrl_position = span.start())
        }
    }
    Ok(Value::Null)
}

#[derive(Clone, Copy, Debug)]
pub struct Log;

impl Function for Log {
    fn identifier(&self) -> &'static str {
        "log"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::ANY,
                required: true,
            },
            Parameter {
                keyword: "level",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "rate_limit_secs",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "default log level (info)",
                source: r#"log("foo")"#,
                result: Ok("null"),
            },
            Example {
                title: "custom level",
                source: r#"log("foo", "error")"#,
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
            .unwrap_or_else(|| "info".into())
            .try_bytes()
            .expect("log level not bytes");
        let rate_limit_secs = arguments.optional("rate_limit_secs");

        Ok(LogFn {
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

#[derive(Debug, Clone)]
struct LogFn {
    span: Span,
    value: Box<dyn Expression>,
    level: Bytes,
    rate_limit_secs: Option<Box<dyn Expression>>,
}

impl FunctionExpression for LogFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let rate_limit_secs = match &self.rate_limit_secs {
            Some(expr) => expr.resolve(ctx)?,
            None => value!(1),
        };

        let span = self.span;

        log(rate_limit_secs, &self.level, value, span)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::null().infallible()
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::*;

    test_function![
        log => Log;

        doesnotcrash {
            args: func_args! [ value: value!(42),
                               level: value!("warn"),
                               rate_limit_secs: value!(5) ],
            want: Ok(Value::Null),
            tdef: TypeDef::null().infallible(),
        }
    ];

    #[traced_test]
    #[test]
    fn output_quotes() {
        // Check that a message is logged without additional quotes
        log(
            value!(1),
            &Bytes::from("warn"),
            value!("simple test message"),
            Default::default(),
        )
        .unwrap();

        assert!(!logs_contain("\"simple test message\""));
        assert!(logs_contain("simple test message"));
    }
}
