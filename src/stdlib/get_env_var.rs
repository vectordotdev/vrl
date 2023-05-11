use crate::compiler::prelude::*;

fn get_env_var(value: Value) -> Resolved {
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

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "name",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "home",
            source: r#"get_env_var!("HOME") != """#,
            result: Ok("true"),
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
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.name.resolve(ctx)?;
        get_env_var(value)
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
            std::env::set_var("VAR2", "var");
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
