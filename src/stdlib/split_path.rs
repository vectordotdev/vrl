use crate::compiler::prelude::*;
use std::path::Path;

fn split_path(path_str: &str) -> Value {
    let path = Path::new(path_str);

    let split_path: Vec<_> = path
        .components()
        .map(|comp| comp.as_os_str().to_string_lossy().into_owned())
        .collect();
    split_path.into()
}

#[derive(Clone, Copy, Debug)]
pub struct SplitPath;

impl Function for SplitPath {
    fn identifier(&self) -> &'static str {
        "split_path"
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

        Ok(SplitPathFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Split path with trailing slash",
                source: r#"split_path!("/home/user/")"#,
                result: Ok(r#"["/", "home", "user"]"#),
            },
            Example {
                title: "Split path from file path",
                source: r#"split_path!("/home/user")"#,
                result: Ok(r#"["/", "home", "user"]"#),
            },
            Example {
                title: "Split path from root",
                source: r#"split_path!("/")"#,
                result: Ok(r#"["/"]"#),
            },
            Example {
                title: "Empty path returns empty array",
                source: r#"split_path!("")"#,
                result: Ok("[]"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct SplitPathFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for SplitPathFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let path_str = value.try_bytes_utf8_lossy()?;
        Ok(split_path(&path_str))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::array(Collection::any())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    fn tdef() -> TypeDef {
        TypeDef::array(Collection::any())
    }

    test_function![
        split_path => SplitPath;

        home_user_trailing_slash {
            args: func_args![value: "/home/user/"],
            want: Ok(value!(["/", "home", "user"])),
            tdef: tdef(),
        }

        home_user_no_trailing_slash {
            args: func_args![value: "/home/user"],
            want: Ok(value!(["/", "home", "user"])),
            tdef: tdef(),
        }

        root {
            args: func_args![value: "/"],
            want: Ok(value!(["/"])),
            tdef: tdef(),
        }

        empty {
            args: func_args![value: ""],
            want: Ok(value!([])),
            tdef: tdef(),
        }

    ];
}
