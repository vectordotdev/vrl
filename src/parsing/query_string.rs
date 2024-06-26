use crate::compiler::prelude::*;
use std::collections::BTreeMap;
use url::form_urlencoded;

pub(crate) fn parse_query_string(bytes: &Bytes, ignore_keys_without_values: bool) -> Resolved {
    let mut query_string = bytes.as_ref();
    if !query_string.is_empty() && query_string[0] == b'?' {
        query_string = &query_string[1..];
    }
    let mut result = BTreeMap::new();
    let parsed = form_urlencoded::parse(query_string);
    for (k, value) in parsed {
        let value = value.as_ref();
        if ignore_keys_without_values && value.is_empty() {
            continue;
        }
        result
            .entry(k.into_owned().into())
            .and_modify(|v| {
                match v {
                    Value::Array(v) => {
                        v.push(value.into());
                    }
                    v => {
                        *v = Value::Array(vec![v.clone(), value.into()]);
                    }
                };
            })
            .or_insert_with(|| value.into());
    }
    Ok(result.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btreemap;

    #[test]
    fn test_parses_complete() {
        let result = parse_query_string(&"foo=%2B1&bar=2&xyz=&abc".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "foo" => "+1",
                "bar" => "2",
                "xyz" => "",
                "abc" => "",
            })
        );
    }

    #[test]
    fn test_parses_multiple_values() {
        let result = parse_query_string(&"foo=bar&foo=xyz".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "foo" => vec!["bar", "xyz"],
            })
        );
    }

    #[test]
    fn test_parses_ruby_on_rails_multiple_values() {
        let result = parse_query_string(&"?foo%5b%5d=bar&foo%5b%5d=xyz".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "foo[]" =>  vec!["bar", "xyz"],
            })
        );
    }

    #[test]
    fn test_parses_empty_key() {
        let result = parse_query_string(&"=&=".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "" => vec!["", ""],
            })
        );
    }

    #[test]
    fn test_parses_single_key() {
        let result = parse_query_string(&"foo".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "foo" => "",
            })
        );
    }

    #[test]
    fn ignores_key_without_values() {
        let result = parse_query_string(&"foo".into(), true).unwrap();
        assert_eq!(result, Value::from(btreemap! {}));
    }

    #[test]
    fn test_parses_empty_string() {
        let result = parse_query_string(&"".into(), false).unwrap();
        assert_eq!(result, Value::from(btreemap! {}));
    }

    #[test]
    fn test_parses_if_starts_with_question_mark() {
        let result = parse_query_string(&"?foo=bar".into(), false).unwrap();
        assert_eq!(
            result,
            Value::from(btreemap! {
                "foo" => "bar",
            })
        );
    }
}
