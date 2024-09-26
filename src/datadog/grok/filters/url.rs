use crate::compiler::prelude::*;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::collections::BTreeMap;
use url::Url;

pub fn parse_url(input: &str) -> Resolved {
    Url::parse(input)
        .map_err(|e| format!("unable to parse url: {e}").into())
        .map(|url| url_to_dd_value(url))
}

fn url_to_dd_value(url: Url) -> Value {
    let mut map = BTreeMap::<&str, Value>::new();

    map.insert("scheme", url.scheme().into());
    map.insert("host", url.host_str().map(ToOwned::to_owned).into());
    map.insert("path", url.path().into());

    if !url.username().is_empty() {
        let mut auth_map = ObjectMap::new();
        auth_map.insert(
            KeyString::from("username"),
            url.username().to_owned().into(),
        );

        if let Some(password) = url.password() {
            auth_map.insert(KeyString::from("password"), password.to_owned().into());
        }

        map.insert("auth", Value::Object(auth_map));
    }

    if let Some(port) = url.port() {
        map.insert("port", port.into());
    };

    if let Some(hash) = url.fragment() {
        map.insert("hash", hash.to_owned().into());
    }

    let query_pairs: ObjectMap = url
        .query_pairs()
        .into_owned()
        .map(|(k, v)| {
            (
                k.into(),
                utf8_percent_encode(&v, NON_ALPHANUMERIC).to_string().into(),
            )
        })
        .collect::<ObjectMap>();

    if !query_pairs.is_empty() {
        let query_string: ObjectMap = query_pairs
            .into_iter()
            .map(|(k, v)| (KeyString::from(k), Value::from(v)))
            .collect();
        map.insert("queryString", query_string.into());
    }

    map.into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect::<Value>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btreemap;

    #[test]
    fn test_parses_simple_url() {
        let result = parse_url("https://vector.dev/".into()).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "scheme" => "https",
                "host" => "vector.dev",
                "path" => "/"
            })
        );
    }

    #[test]
    fn test_parses_url_with_query_strings() {
        let result = parse_url(
            "https://help.datadoghq.com/hc/en-us/search?utf8=%E2%9C%93&query=install&commit=Search"
                .into(),
        )
        .unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "scheme" => "https",
                "host" => "help.datadoghq.com",
                "path" => "/hc/en-us/search",
                "queryString" => btreemap!{
                    "utf8" => "%E2%9C%93",
                    "query" => "install",
                    "commit" => "Search"
                },
            })
        );
    }

    #[test]
    fn test_parses_complex_url() {
        let result = parse_url("https://user:password@api.logmatic.io:8080/a/long/path/file.txt?debug&param1=foo&param2=bar#!/super/hash".into()).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "scheme" => "https",
                "host" => "api.logmatic.io",
                "port" => 8080,
                "path" => "/a/long/path/file.txt",
                "queryString" => btreemap! {
                    "debug" => "",
                    "param1" => "foo",
                    "param2" => "bar"
                },
                "auth" => btreemap! {
                "username" => "user",
                "password" => "password"
                },
                "hash" => "!/super/hash"
            })
        );
    }

    // Url::parse only works on absolute URLs (at least scheme + host)
    // Diff with the logs implementation, which is able to parse relative URLs
    #[test]
    fn test_parse_err_relative_url() {
        let result = parse_url("/youpi1/youpi2/img.jpg?q=my%20query#configure/input".into());
        assert!(result.is_err());
    }

    // Diff with the logs implementation, which returns an empty string for path
    #[test]
    fn test_parse_no_path() {
        let result = parse_url("http://j.mp".into()).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "scheme" => "http",
                "host" => "j.mp",
                "path" => "/"
            })
        );
    }
}
