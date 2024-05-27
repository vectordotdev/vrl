use crate::compiler::prelude::*;
use std::collections::BTreeMap;
use url::form_urlencoded;

pub(crate) fn parse_query_string(bytes: &Bytes) -> Resolved {
    let mut query_string = bytes.as_ref();
    if !query_string.is_empty() && query_string[0] == b'?' {
        query_string = &query_string[1..];
    }
    let mut result = BTreeMap::new();
    let parsed = form_urlencoded::parse(query_string);
    for (k, value) in parsed {
        let value = value.as_ref();
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

