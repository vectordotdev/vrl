use std::collections::BTreeMap;

use chrono::DateTime;
use influxdb_line_protocol::{FieldValue, ParsedLine};

use crate::compiler::prelude::*;

fn influxdb_lines_to_metrics(line: ParsedLine) -> Vec<ObjectMap> {
    let ParsedLine {
        series,
        field_set,
        timestamp,
    } = line;

    let timestamp = timestamp.map(DateTime::from_timestamp_nanos);

    let tags: Option<ObjectMap> = series.tag_set.as_ref().map(|tags| {
        tags.iter()
            .map(|t| (t.0.to_string().into(), t.1.to_string().into()))
            .collect()
    });

    field_set
        .iter()
        .filter_map(|f| {
            let mut metric = ObjectMap::new();
            let measurement = &series.measurement;
            let field_key = f.0.to_string();
            let field_value = match f.1 {
                FieldValue::I64(v) => v as f64,
                FieldValue::U64(v) => v as f64,
                FieldValue::F64(v) => v,
                FieldValue::Boolean(v) => {
                    if v {
                        1.0
                    } else {
                        0.0
                    }
                }
                // TODO: return none or return Error?
                FieldValue::String(_) => return None,
            };

            // influxdb_line_protocol crate seems to not allow NaN float values while parsing
            // field values and this case should not happen, but just in case, we should
            // ignore the field.
            let Ok(field_value) = NotNan::new(field_value) else {
                // TODO: return none or return error?
                return None;
            };

            let metric_name = format!("{measurement}_{field_key}");
            metric.insert("name".into(), metric_name.into());

            if let Some(tags) = tags.as_ref() {
                metric.insert("tags".into(), tags.clone().into());
            }

            if let Some(timestamp) = timestamp {
                metric.insert("timestamp".into(), timestamp.into());
            }

            metric.insert("kind".into(), "absolute".into());

            let gauge_object: ObjectMap = [("value".into(), field_value.into())].into();
            metric.insert("gauge".into(), gauge_object.into());

            Some(metric)
        })
        .collect()
}

fn parse_influxdb(bytes: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let line = String::from_utf8_lossy(&bytes);
    let parsed_line = influxdb_line_protocol::parse_lines(&line);

    let metrics = parsed_line
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(influxdb_lines_to_metrics)
        .map(Value::from);

    Ok(Value::Array(metrics.collect()))
}

impl From<influxdb_line_protocol::Error> for ExpressionError {
    fn from(error: influxdb_line_protocol::Error) -> Self {
        Self::Error {
            message: format!("InfluxDB line protocol parsing error: {error}"),
            labels: vec![],
            notes: vec![],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseInfluxDB;

impl Function for ParseInfluxDB {
    fn identifier(&self) -> &'static str {
        "parse_influxdb"
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

        Ok(ParseInfluxDBFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        // TODO: add examples
        &[]
    }
}

#[derive(Clone, Debug)]
struct ParseInfluxDBFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseInfluxDBFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        parse_influxdb(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        type_def()
    }
}

fn tags_kind() -> Kind {
    Kind::object(Collection::from_unknown(Kind::bytes()))
}

fn gauge_kind() -> Kind {
    Kind::object(BTreeMap::from([("value".into(), Kind::float())]))
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("name".into(), Kind::bytes()),
        ("tags".into(), tags_kind()),
        ("timestamp".into(), Kind::timestamp()),
        ("kind".into(), Kind::bytes()),
        ("gauge".into(), gauge_kind()),
    ])
}

fn type_def() -> TypeDef {
    TypeDef::object(inner_kind()).fallible()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::btreemap;

    const TIMESTAMP_NANOS: i64 = 1590488773254420000;

    test_function![
        parse_influxdb => ParseInfluxDB;

        influxdb_valid {
            args: func_args![ value: format!("cpu,host=A,region=us-west usage_system=64i,usage_user=10u,temperature=50.5,on=true,sleep=false {TIMESTAMP_NANOS}") ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 64.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_usage_user",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 10.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_temperature",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 50.5,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_on",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 1.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_sleep",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 0.0,
                    },
                }),
            ])),
            tdef: type_def(),
        }


        influxdb_valid_no_timestamp {
            args: func_args![ value: "cpu,host=A,region=us-west usage_system=64i,usage_user=10i" ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 64.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_usage_user",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 10.0,
                    },
                }),
            ])),
            tdef: type_def(),
        }

        influxdb_valid_no_tags {
            args: func_args![ value: format!("cpu usage_system=64i,usage_user=10i {TIMESTAMP_NANOS}") ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 64.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_usage_user",
                    "timestamp" => DateTime::from_timestamp_nanos(TIMESTAMP_NANOS),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 10.0,
                    },
                }),
            ])),
            tdef: type_def(),
        }

        influxdb_valid_no_tags_no_timestamp {
            args: func_args![ value: "cpu usage_system=64i,usage_user=10i" ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 64.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_usage_user",
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 10.0,
                    },
                }),
            ])),
            tdef: type_def(),
        }

        influxdb_valid_ignores_string_value {
            args: func_args![ value: r#"valid foo="bar""# ],
            want: Ok(Value::Array(vec![])),
            tdef: type_def(),
        }

        influxdb_invalid_no_fields{
            args: func_args![ value: "cpu " ],
            want: Err("InfluxDB line protocol parsing error: No fields were provided"),
            tdef: type_def(),
        }
    ];
}
