use crate::compiler::prelude::*;
use std::path::Path;

fn dirname(path_str: &str) -> &str {
    if path_str == "/" {
        return "/";
    }

    let path = Path::new(path_str);
    match path.parent() {
        Some(parent) => {
            if parent.as_os_str().is_empty() {
                "."
            } else {
                parent.to_str().unwrap_or(".")
            }
        }
        None => ".",
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirName;

impl Function for DirName {
    fn identifier(&self) -> &'static str {
        "dirname"
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

        Ok(DirNameFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Extract dirname from file path",
                source: r#"dirname!("/usr/local/bin/vrl")"#,
                result: Ok("\"/usr/local/bin\""),
            },
            Example {
                title: "Extract dirname from file path with extension",
                source: r#"dirname!("/home/user/file.txt")"#,
                result: Ok("\"/home/user\""),
            },
            Example {
                title: "Extract dirname from directory path",
                source: r#"dirname!("/home/user/")"#,
                result: Ok("\"/home\""),
            },
            Example {
                title: "Root directory dirname is itself",
                source: r#"dirname!("/")"#,
                result: Ok("\"/\""),
            },
            Example {
                title: "Relative files have current directory as dirname",
                source: r#"dirname!("file.txt")"#,
                result: Ok("\".\""),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct DirNameFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DirNameFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let path_str_cow = value.try_bytes_utf8_lossy()?;
        let path_str = path_str_cow.as_ref();
        Ok(Value::from(dirname(path_str)))
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
        dirname => DirName;

        home_user_trailing_slash {
            args: func_args![value: "/home/user/"],
            want: Ok("/home"),
            tdef: tdef(),
        }

        home_user_no_trailing_slash {
            args: func_args![value: "/home/user"],
            want: Ok("/home"),
            tdef: tdef(),
        }

        root {
            args: func_args![value: "/"],
            want: Ok("/"),
            tdef: tdef(),
        }

        current_dir {
            args: func_args![value: "."],
            want: Ok("."),
            tdef: tdef(),
        }

        parent_dir {
            args: func_args![value: ".."],
            want: Ok("."),
            tdef: tdef(),
        }

        file_in_current_dir {
            args: func_args![value: "file"],
            want: Ok("."),
            tdef: tdef(),
        }

        hidden_file {
            args: func_args![value: ".file"],
            want: Ok("."),
            tdef: tdef(),
        }

        empty_string {
            args: func_args![value: ""],
            want: Ok("."),
            tdef: tdef(),
        }

        double_extension {
            args: func_args![value: "file.tar.gz"],
            want: Ok("."),
            tdef: tdef(),
        }

        path_with_extension {
            args: func_args![value: "/home/user/file.txt"],
            want: Ok("/home/user"),
            tdef: tdef(),
        }
    ];
}
