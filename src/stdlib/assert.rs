use crate::compiler::prelude::*;

fn assert(condition: Value, message: Option<Value>, format: Option<String>) -> Resolved {
    if condition.try_boolean()? {
        Ok(true.into())
    } else if let Some(message) = message {
        let message = message.try_bytes_utf8_lossy()?.into_owned();
        Err(ExpressionError::Error {
            message: message.clone(),
            labels: vec![],
            notes: vec![Note::UserErrorMessage(message)],
        })
    } else {
        let message = match format {
            Some(string) => format!("assertion failed: {string}"),
            None => "assertion failed".to_owned(),
        };
        Err(ExpressionError::from(message))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Assert;

impl Function for Assert {
    fn identifier(&self) -> &'static str {
        "assert"
    }

    fn usage(&self) -> &'static str {
        "Asserts the `condition`, which must be a Boolean expression. The program is aborted with `message` if the condition evaluates to `false`."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "condition",
                kind: kind::BOOLEAN,
                required: true,
                description: "The condition to check.",
                default: None,
            },
            Parameter {
                keyword: "message",
                kind: kind::BYTES,
                required: false,
                description:
                    "An optional custom error message. If the equality assertion fails, `message` is
appended to the default message prefix. See the [examples](#assert-examples) below
for a fully formed log message sample.",
                default: None,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Assertion (true) - with message",
                source: r#"assert!("foo" == "foo", message: "\"foo\" must be \"foo\"!")"#,
                result: Ok("true"),
            },
            example! {
                title: "Assertion (false) - with message",
                source: r#"assert!("foo" == "bar", message: "\"foo\" must be \"foo\"!")"#,
                result: Err(r#"function call error for "assert" at (0:60): "foo" must be "foo"!"#),
            },
            example! {
                title: "Assertion (false) - simple",
                source: "assert!(false)",
                result: Err(r#"function call error for "assert" at (0:14): assertion failed"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let condition = arguments.required("condition");
        let message = arguments.optional("message");

        Ok(AssertFn { condition, message }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct AssertFn {
    condition: Box<dyn Expression>,
    message: Option<Box<dyn Expression>>,
}

impl FunctionExpression for AssertFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let condition = self.condition.resolve(ctx)?;
        let format = self.condition.format();
        let message = self.message.as_ref().map(|m| m.resolve(ctx)).transpose()?;

        assert(condition, message, format)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().fallible()
    }
}

impl fmt::Display for AssertFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("")
    }
}
