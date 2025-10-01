use crate::compiler::prelude::*;
use std::path::Path;

fn basename(value: &Value) -> Resolved {
    let path_str_cow = value.try_bytes_utf8_lossy()?;
    let path_str = path_str_cow.as_ref();
    let path = Path::new(path_str);

    let basename = path.file_name().and_then(|s| s.to_str()).map(Value::from);
    Ok(basename.into())
}

#[derive(Clone, Copy, Debug)]
pub struct BaseName;

impl Function for BaseName {
    fn identifier(&self) -> &'static str {
        "basename"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(BaseNameFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Extract basename from file path",
                source: r#"basename!("/usr/local/bin/vrl")"#,
                result: Ok("\"vrl\""),
            },
            Example {
                title: "Extract basename from file path with extension",
                source: r#"basename!("/home/user/file.txt")"#,
                result: Ok("\"file.txt\""),
            },
            Example {
                title: "Extract basename from directory path",
                source: r#"basename!("/home/user/")"#,
                result: Ok("\"user\""),
            },
            Example {
                title: "Root directory has no basename",
                source: r#"basename!("/")"#,
                result: Ok("null"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct BaseNameFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for BaseNameFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        basename(&value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn tdef() -> TypeDef {
        TypeDef::bytes().fallible()
    }

    test_function![
        basename => BaseName;

        home_user_trailing_slash {
            args: func_args![value: "/home/user/"],
            want: Ok("user"),
            tdef: tdef(),
        }

        home_user_no_trailing_slash {
            args: func_args![value: "/home/user"],
            want: Ok("user"),
            tdef: tdef(),
        }

        root {
            args: func_args![value: "/"],
            want: Ok(Value::Null),
            tdef: tdef(),
        }

        current_dir {
            args: func_args![value: "."],
            want: Ok(Value::Null),
            tdef: tdef(),
        }

        parent_dir {
            args: func_args![value: ".."],
            want: Ok(Value::Null),
            tdef: tdef(),
        }

        file_in_current_dir {
            args: func_args![value: "file"],
            want: Ok("file"),
            tdef: tdef(),
        }

        hidden_file {
            args: func_args![value: ".file"],
            want: Ok(".file"),
            tdef: tdef(),
        }

        double_extension {
            args: func_args![value: "file.tar.gz"],
            want: Ok("file.tar.gz"),
            tdef: tdef(),
        }

        path_with_extension {
            args: func_args![value: "/home/user/file.txt"],
            want: Ok("file.txt"),
            tdef: tdef(),
        }
    ];
}
