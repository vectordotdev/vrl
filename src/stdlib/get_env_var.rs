use crate::compiler::prelude::*;

fn get_env_var(value: &Value) -> Resolved {
    let name = value.try_bytes_utf8_lossy()?;
    std::env::var(name.as_ref())
        .map(Into::into)
        .map_err(|e| e.to_string().into())
}

#[derive(Clone, Copy, Debug)]
pub struct GetEnvVar;

impl Function for GetEnvVar {
    fn identifier(&self) -> &'static str {
        "get_env_var"
    }

    fn usage(&self) -> &'static str {
        "Returns the value of the environment variable specified by `name`."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "Environment variable `name` does not exist.",
            "The value of environment variable `name` is not valid Unicode",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "name",
            kind: kind::BYTES,
            required: true,
            description: "The name of the environment variable.",
            default: None,
        }]
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get an environment variable",
            source: r#"get_env_var!("HOME") != """#,
            result: Ok("true"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get an environment variable",
            source: r#"get_env_var!("HOME")"#,
            result: Ok("/root"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let name = arguments.required("name");

        Ok(GetEnvVarFn { name }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct GetEnvVarFn {
    name: Box<dyn Expression>,
}

impl FunctionExpression for GetEnvVarFn {
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.name.resolve(ctx)?;
        get_env_var(&value)
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.name.resolve(ctx)?;
        let name = value.try_bytes_utf8_lossy()?;
        if name.as_ref() == "HOME" {
            Ok("/root".into())
        } else {
            get_env_var(&value)
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        get_env_var => GetEnvVar;

        before_each => {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("VAR2", "var") };
        }

        doesnt_exist {
            args: func_args![name: "VAR1"],
            want: Err("environment variable not found"),
            tdef: TypeDef::bytes().fallible(),
        }

        exists {
            args: func_args![name: "VAR2"],
            want: Ok(value!("var")),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid1 {
            args: func_args![name: "="],
            want: Err("environment variable not found"),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid2 {
            args: func_args![name: ""],
            want: Err("environment variable not found"),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid3 {
            args: func_args![name: "a=b"],
            want: Err("environment variable not found"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
