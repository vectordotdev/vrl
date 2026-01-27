use crate::compiler::prelude::*;
use std::collections::BTreeMap;
use std::sync::LazyLock;
use url::Url;

static DEFAULT_DEFAULT_KNOWN_PORTS: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The text of the URL.",
            default: None,
        },
        Parameter {
            keyword: "default_known_ports",
            kind: kind::BOOLEAN,
            required: false,
            description: "If true and the port number is not specified in the input URL
string (or matches the default port for the scheme), it is
populated from well-known ports for the following schemes:
`http`, `https`, `ws`, `wss`, and `ftp`.",
            default: Some(&DEFAULT_DEFAULT_KNOWN_PORTS),
        },
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct ParseUrl;

impl Function for ParseUrl {
    fn identifier(&self) -> &'static str {
        "parse_url"
    }

    fn usage(&self) -> &'static str {
        "Parses the `value` in [URL](https://en.wikipedia.org/wiki/URL) format."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a properly formatted URL."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse URL",
                source: r#"parse_url!("ftp://foo:bar@example.com:4343/foobar?hello=world#123")"#,
                result: Ok(indoc! {r#"
                {
                    "fragment": "123",
                    "host": "example.com",
                    "password": "bar",
                    "path": "/foobar",
                    "port": 4343,
                    "query": {
                        "hello": "world"
                    },
                    "scheme": "ftp",
                    "username": "foo"
                }
            "#}),
            },
            example! {
                title: "Parse URL with default port",
                source: r#"parse_url!("https://example.com", default_known_ports: true)"#,
                result: Ok(indoc! {r#"
                {
                    "fragment": null,
                    "host": "example.com",
                    "password": "",
                    "path": "/",
                    "port": 443,
                    "query": {},
                    "scheme": "https",
                    "username": ""
                }
            "#}),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let default_known_ports = arguments.optional("default_known_ports");

        Ok(ParseUrlFn {
            value,
            default_known_ports,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseUrlFn {
    value: Box<dyn Expression>,
    default_known_ports: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseUrlFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let default_known_ports = self
            .default_known_ports
            .map_resolve_with_default(ctx, || DEFAULT_DEFAULT_KNOWN_PORTS.clone())?
            .try_boolean()?;

        Url::parse(&string)
            .map_err(|e| format!("unable to parse url: {e}").into())
            .map(|url| url_to_value(&url, default_known_ports))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).fallible()
    }
}

fn url_to_value(url: &Url, default_known_ports: bool) -> Value {
    let mut map = BTreeMap::<&str, Value>::new();

    map.insert("scheme", url.scheme().to_owned().into());
    map.insert("username", url.username().to_owned().into());
    map.insert(
        "password",
        url.password()
            .map(ToOwned::to_owned)
            .unwrap_or_default()
            .into(),
    );
    map.insert("path", url.path().to_owned().into());
    map.insert("host", url.host_str().map(ToOwned::to_owned).into());

    let port = if default_known_ports {
        url.port_or_known_default()
    } else {
        url.port()
    };
    map.insert("port", port.into());
    map.insert("fragment", url.fragment().map(ToOwned::to_owned).into());
    map.insert(
        "query",
        url.query_pairs()
            .into_owned()
            .map(|(k, v)| (k.into(), v.into()))
            .collect::<ObjectMap>()
            .into(),
    );

    map.into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect::<Value>()
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("scheme".into(), Kind::bytes()),
        ("username".into(), Kind::bytes()),
        ("password".into(), Kind::bytes()),
        ("path".into(), Kind::bytes().or_null()),
        ("host".into(), Kind::bytes()),
        ("port".into(), Kind::integer().or_null()),
        ("fragment".into(), Kind::bytes().or_null()),
        (
            "query".into(),
            Kind::object(Collection::from_unknown(Kind::bytes())),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        parse_url => ParseUrl;

        https {
            args: func_args![value: value!("https://vector.dev")],
            want: Ok(value!({
                fragment: (),
                host: "vector.dev",
                password: "",
                path: "/",
                port: (),
                query: {},
                scheme: "https",
                username: "",
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        default_port_specified {
            args: func_args![value: value!("https://vector.dev:443")],
            want: Ok(value!({
                fragment: (),
                host: "vector.dev",
                password: "",
                path: "/",
                port: (),
                query: {},
                scheme: "https",
                username: "",
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        default_port {
            args: func_args![value: value!("https://vector.dev"), default_known_ports: true],
            want: Ok(value!({
                fragment: (),
                host: "vector.dev",
                password: "",
                path: "/",
                port: 443_i64,
                query: {},
                scheme: "https",
                username: "",
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        punycode {
            args: func_args![value: value!("https://www.café.com")],
            want: Ok(value!({
                fragment: (),
                host: "www.xn--caf-dma.com",
                password: "",
                path: "/",
                port: (),
                query: {},
                scheme: "https",
                username: "",
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        punycode_mixed_case {
            args: func_args![value: value!("https://www.CAFé.com")],
            want: Ok(value!({
                fragment: (),
                host: "www.xn--caf-dma.com",
                password: "",
                path: "/",
                port: (),
                query: {},
                scheme: "https",
                username: "",
            })),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
