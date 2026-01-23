use crate::compiler::prelude::*;

fn assert_eq(left: &Value, right: &Value, message: Option<Value>) -> Resolved {
    if left == right {
        Ok(true.into())
    } else if let Some(message) = message {
        let message = message.try_bytes_utf8_lossy()?.into_owned();
        Err(ExpressionError::Error {
            message: message.clone(),
            labels: vec![],
            notes: vec![Note::UserErrorMessage(message)],
        })
    } else {
        Err(ExpressionError::from(format!(
            "assertion failed: {left} == {right}"
        )))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AssertEq;

impl Function for AssertEq {
    fn identifier(&self) -> &'static str {
        "assert_eq"
    }

    fn usage(&self) -> &'static str {
        "Asserts that two expressions, `left` and `right`, have the same value. The program is aborted with `message` if they do not have the same value."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "left",
                kind: kind::ANY,
                required: true,
                description: "The value to check for equality against `right`.",
            },
            Parameter {
                keyword: "right",
                kind: kind::ANY,
                required: true,
                description: "The value to check for equality against `left`.",
            },
            Parameter {
                keyword: "message",
                kind: kind::BYTES,
                required: false,
                description:
                    "An optional custom error message. If the equality assertion fails, `message` is
appended to the default message prefix. See the [examples](#assert_eq-examples)
below for a fully formed log message sample.",
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Successful assertion",
                source: "assert_eq!(1, 1)",
                result: Ok("true"),
            },
            example! {
                title: "Unsuccessful assertion",
                source: "assert_eq!(127, [1, 2, 3])",
                result: Err(
                    r#"function call error for "assert_eq" at (0:26): assertion failed: 127 == [1, 2, 3]"#,
                ),
            },
            example! {
                title: "Unsuccessful assertion with custom log message",
                source: r#"assert_eq!(1, 0, message: "Unequal integers")"#,
                result: Err(r#"function call error for "assert_eq" at (0:45): Unequal integers"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let left = arguments.required("left");
        let right = arguments.required("right");
        let message = arguments.optional("message");

        Ok(AssertEqFn {
            left,
            right,
            message,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct AssertEqFn {
    left: Box<dyn Expression>,
    right: Box<dyn Expression>,
    message: Option<Box<dyn Expression>>,
}

impl FunctionExpression for AssertEqFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let left = self.left.resolve(ctx)?;
        let right = self.right.resolve(ctx)?;
        let message = self.message.as_ref().map(|m| m.resolve(ctx)).transpose()?;

        assert_eq(&left, &right, message)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        assert_eq => AssertEq;

        pass {
            args: func_args![left: "foo", right: "foo"],
            want: Ok(true),
            tdef: TypeDef::boolean().fallible(),
        }

        fail {
            args: func_args![left: "foo", right: "bar"],
            want: Err(r#"assertion failed: "foo" == "bar""#),
            tdef: TypeDef::boolean().fallible(),
        }

        message {
            args: func_args![left: "foo", right: "bar", message: "failure!"],
            want: Err("failure!"),
            tdef: TypeDef::boolean().fallible(),
        }
    ];
}
