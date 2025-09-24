use crate::btreemap;
use crate::compiler::prelude::*;
use crate::value::ObjectMap;
use std::path::Path;

fn parse_path(value: &Value) -> Resolved {
    let path_str_cow = value.try_bytes_utf8_lossy()?;
    let path_str = path_str_cow.as_ref();

    // strip the trailing slash if it exists
    let path = if path_str.len() > 1 && path_str.ends_with('/') {
        Path::new(&path_str[..path_str.len() - 1])
    } else {
        Path::new(path_str)
    };

    let mut result = ObjectMap::new();

    let basename = path.file_name().and_then(|s| s.to_str()).map(Value::from);
    result.insert("basename".into(), basename.clone().into());

    let components_path = if basename.is_some() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let directories: Vec<Value> = components_path
        .components()
        .map(|comp| Value::from(comp.as_os_str().to_string_lossy().as_ref()))
        .collect();
    result.insert("directories".into(), Value::Array(directories));

    let extension = path.extension().and_then(|s| s.to_str()).map(Value::from);
    result.insert("extension".into(), extension.into());

    Ok(result.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ParsePath;

impl Function for ParsePath {
    fn identifier(&self) -> &'static str {
        "parse_path"
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

        Ok(ParsePathFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Parse a file path",
                source: r#"parse_path!("/usr/local/bin/vrl")"#,
                result: Ok(indoc! { r#"{
                    "basename": "vrl",
                    "directories": ["/", "usr", "local", "bin"],
                    "extension": null
                }"# }),
            },
            Example {
                title: "Parse a file path with extension",
                source: r#"parse_path!("/home/user/file.txt")"#,
                result: Ok(indoc! { r#"{
                    "basename": "file.txt",
                    "directories": ["/", "home", "user"],
                    "extension": "txt"
                }"# }),
            },
            Example {
                title: "Parse a directory path",
                source: r#"parse_path!("/home/user/")"#,
                result: Ok(indoc! { r#"{
                    "basename": "user",
                    "directories": ["/", "home"],
                    "extension": null
                }"# }),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ParsePathFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParsePathFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_path(&value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(btreemap! {
            Field::from("basename") => Kind::bytes() | Kind::null(),
            Field::from("directories") => Kind::array(Collection::from_unknown(Kind::bytes())),
            Field::from("extension") => Kind::bytes() | Kind::null(),
        })
        .fallible()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    fn tdef() -> TypeDef {
        TypeDef::object(btreemap! {
            Field::from("basename") => Kind::bytes() | Kind::null(),
            Field::from("directories") => Kind::array(Collection::from_unknown(Kind::bytes())),
            Field::from("extension") => Kind::bytes() | Kind::null(),
        })
        .fallible()
    }

    test_function![
        parse_path => ParsePath;

        home_user_trailing_slash {
            args: func_args![value: "/home/user/"],
            want: Ok(value!({
                "basename": "user",
                "directories": ["/", "home"],
                "extension": null,
            })),
            tdef: tdef(),
        }

        home_user_no_trailing_slash {
            args: func_args![value: "/home/user"],
            want: Ok(value!({
                "basename": "user",
                "directories": ["/", "home"],
                "extension": null,
            })),
            tdef: tdef(),
        }

        root {
            args: func_args![value: "/"],
            want: Ok(value!({
                "basename": null,
                "directories": ["/"],
                "extension": null,
            })),
            tdef: tdef(),
        }

        current_dir {
            args: func_args![value: "."],
            want: Ok(value!({
                "basename": null,
                "directories": ["."],
                "extension": null,
            })),
            tdef: tdef(),
        }

        parent_dir {
            args: func_args![value: ".."],
            want: Ok(value!({
                "basename": null,
                "directories": [".."],
                "extension": null,
            })),
            tdef: tdef(),
        }

        file_in_current_dir {
            args: func_args![value: "file"],
            want: Ok(value!({
                "basename": "file",
                "directories": [],
                "extension": null,
            })),
            tdef: tdef(),
        }

        hidden_file {
            args: func_args![value: ".file"],
            want: Ok(value!({
                "basename": ".file",
                "directories": [],
                "extension": null,
            })),
            tdef: tdef(),
        }

        double_extension {
            args: func_args![value: "file.tar.gz"],
            want: Ok(value!({
                "basename": "file.tar.gz",
                "directories": [],
                "extension": "gz",
            })),
            tdef: tdef(),
        }

        path_with_extension {
            args: func_args![value: "/home/user/file.txt"],
            want: Ok(value!({
                "basename": "file.txt",
                "directories": ["/", "home", "user"],
                "extension": "txt",
            })),
            tdef: tdef(),
        }
    ];
}
