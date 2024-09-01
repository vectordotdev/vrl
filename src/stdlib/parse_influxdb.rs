use std::collections::BTreeMap;

use chrono::DateTime;
use influxdb_line_protocol::{FieldValue, ParsedLine};

use crate::compiler::prelude::*;
use crate::{btreemap, value};

fn influxdb_line_to_metrics(line: ParsedLine) -> Result<Vec<ObjectMap>, ExpressionError> {
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
        .into_iter()
        .map(|f| {
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
                FieldValue::String(_) => {
                    return Err(Error::StringFieldSetValuesNotSupported.into());
                }
            };

            // `influxdb_line_protocol` crate seems to not allow NaN float values while parsing
            // field values and this case should not happen, but just in case, we should
            // handle it.
            let Ok(field_value) = NotNan::new(field_value) else {
                return Err(Error::NaNFieldSetValuesNotSupported.into());
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

            let gauge_object = value!({
                value: field_value
            });
            metric.insert("gauge".into(), gauge_object);

            Ok(metric)
        })
        .collect()
}

#[derive(Debug, Clone, thiserror::Error)]
enum Error {
    #[error("field set values of type string are not supported")]
    StringFieldSetValuesNotSupported,
    #[error("NaN field set values are not supported")]
    NaNFieldSetValuesNotSupported,
}

impl From<Error> for ExpressionError {
    fn from(error: Error) -> Self {
        Self::Error {
            message: format!("Error while converting InfluxDB line protocol metric to Vector's metric model: {error}"),
            labels: vec![],
            notes: vec![],
        }
    }
}

fn parse_influxdb(bytes: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let line = String::from_utf8_lossy(&bytes);
    let parsed_line = influxdb_line_protocol::parse_lines(&line);

    let metrics = parsed_line
        .into_iter()
        .map(|line_result| line_result.map_err(ExpressionError::from))
        .map(|line_result| line_result.and_then(influxdb_line_to_metrics))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .map(Value::from)
        .collect();

    Ok(Value::Array(metrics))
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

    fn summary(&self) -> &'static str {
        "parse an InfluxDB line protocol string into a list of vector-compatible metrics"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse influxdb line protocol",
            source: r#"parse_influxdb!("cpu,host=A,region=us-west usage_system=64i,usage_user=10u,temperature=50.5,on=true,sleep=false 1590488773254420000")"#,
            result: Ok(indoc! {r#"
                [
                    {
                        "name": "cpu_usage_system",
                        "tags": {
                            "host": "A",
                            "region": "us-west"
                        },
                        "timestamp": "2020-05-26T10:26:13.254420Z",
                        "kind": "absolute",
                        "gauge": {
                            "value": 64.0
                        }
                    },
                    {
                        "name": "cpu_usage_user",
                        "tags": {
                            "host": "A",
                            "region": "us-west"
                        },
                        "timestamp": "2020-05-26T10:26:13.254420Z",
                        "kind": "absolute",
                        "gauge": {
                            "value": 10.0
                        }
                    },
                    {
                        "name": "cpu_temperature",
                        "tags": {
                            "host": "A",
                            "region": "us-west"
                        },
                        "timestamp": "2020-05-26T10:26:13.254420Z",
                        "kind": "absolute",
                        "gauge": {
                            "value": 50.5
                        }
                    },
                    {
                        "name": "cpu_on",
                        "tags": {
                            "host": "A",
                            "region": "us-west"
                        },
                        "timestamp": "2020-05-26T10:26:13.254420Z",
                        "kind": "absolute",
                        "gauge": {
                            "value": 1.0
                        }
                    },
                    {
                        "name": "cpu_sleep",
                        "tags": {
                            "host": "A",
                            "region": "us-west"
                        },
                        "timestamp": "2020-05-26T10:26:13.254420Z",
                        "kind": "absolute",
                        "gauge": {
                            "value": 0.0
                        }
                    }
                ]
            "#}),
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
    Kind::object(Collection::from_unknown(Kind::bytes())) | Kind::null()
}

fn gauge_kind() -> Kind {
    Kind::object(btreemap! {
        "value" => Kind::float(),
    })
}

fn metric_kind() -> BTreeMap<Field, Kind> {
    btreemap! {
        "name" => Kind::bytes(),
        "tags" => tags_kind(),
        "timestamp" => Kind::timestamp() | Kind::null(),
        "kind" => Kind::bytes(),
        "gauge" => gauge_kind(),
    }
}

fn inner_kind() -> Kind {
    Kind::object(metric_kind())
}

fn type_def() -> TypeDef {
    TypeDef::array(Collection::from_unknown(inner_kind())).fallible()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::btreemap;

    test_function![
        parse_influxdb => ParseInfluxDB;

        influxdb_valid {
            args: func_args![ value: "cpu,host=A,region=us-west usage_system=64i,usage_user=10u,temperature=50.5,on=true,sleep=false 1590488773254420000" ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "tags" => btreemap! {
                        "host" => "A",
                        "region" => "us-west",
                    },
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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
            args: func_args![ value: "cpu usage_system=64i,usage_user=10i 1590488773254420000" ],
            want: Ok(Value::from(vec![
                Value::from(btreemap! {
                    "name" => "cpu_usage_system",
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
                    "kind" => "absolute",
                    "gauge" => btreemap! {
                        "value" => 64.0,
                    },
                }),
                Value::from(btreemap! {
                    "name" => "cpu_usage_user",
                    "timestamp" => DateTime::from_timestamp_nanos(1_590_488_773_254_420_000),
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

        influxdb_invalid_string_field_set_value {
            args: func_args![ value: r#"valid foo="bar""# ],
            want: Err("Error while converting InfluxDB line protocol metric to Vector's metric model: field set values of type string are not supported"),
            tdef: type_def(),
        }

        influxdb_invalid_no_fields{
            args: func_args![ value: "cpu " ],
            want: Err("InfluxDB line protocol parsing error: No fields were provided"),
            tdef: type_def(),
        }
    ];
}
