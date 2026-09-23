use std::{convert::TryFrom, fmt, num::FpCategory, string::ToString};

use crate::compiler::prelude::Bytes;
use crate::compiler::value::VrlValueConvert;
use crate::parsing::query_string::parse_query_string;
use crate::parsing::ruby_hash::parse_ruby_hash;
use crate::parsing::xml::{ParseOptions, parse_xml};
use crate::value::Value;
use percent_encoding::percent_decode;

use super::{
    ast::{Function, FunctionArgument},
    filters::{array, keyvalue, keyvalue::KeyValueFilter},
    matchers::date::{DateFilter, apply_date_filter},
    parse_grok::InternalError,
    parse_grok_rules::Error as GrokStaticError,
};

#[derive(Debug, Clone)]
pub enum GrokFilter {
    Date(DateFilter),
    Integer,
    IntegerExt,
    // with scientific notation support, e.g. 1e10
    Number,
    NumberExt,
    // with scientific notation support, e.g. 1.52e10
    NullIf(String),
    Scale(f64),
    Lowercase,
    Uppercase,
    Json,
    Rubyhash,
    Querystring,
    Boolean,
    Decodeuricomponent,
    Xml,
    Array(
        Option<(String, String)>,
        Option<String>,
        Box<Option<GrokFilter>>,
    ),
    KeyValue(KeyValueFilter),
}

impl fmt::Display for GrokFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrokFilter::Date(..) => f.pad("Date(..)"),
            GrokFilter::Integer => f.pad("Integer"),
            GrokFilter::IntegerExt => f.pad("IntegerExt"),
            GrokFilter::Number => f.pad("Number"),
            GrokFilter::NumberExt => f.pad("NumberExt"),
            GrokFilter::NullIf(..) => f.pad("NullIf(..)"),
            GrokFilter::Scale(..) => f.pad("Scale(..)"),
            GrokFilter::Lowercase => f.pad("Lowercase"),
            GrokFilter::Uppercase => f.pad("Uppercase"),
            GrokFilter::Json => f.pad("Json"),
            GrokFilter::Rubyhash => f.pad("RubyHash"),
            GrokFilter::Querystring => f.pad("QueryString"),
            GrokFilter::Boolean => f.pad("Boolean"),
            GrokFilter::Decodeuricomponent => f.pad("DecodeUriComponent"),
            GrokFilter::Xml => f.pad("Xml"),
            GrokFilter::Array(..) => f.pad("Array(..)"),
            GrokFilter::KeyValue(..) => f.pad("KeyValue(..)"),
        }
    }
}

impl TryFrom<&Function> for GrokFilter {
    type Error = GrokStaticError;

    fn try_from(f: &Function) -> Result<Self, Self::Error> {
        match f.name.as_str() {
            "scale" => match f.args.as_ref() {
                Some(args) if !args.is_empty() => {
                    let scale_factor = match args[0] {
                        FunctionArgument::Arg(Value::Integer(scale_factor)) => {
                            i64_to_f64(scale_factor)
                        }
                        FunctionArgument::Arg(Value::Float(scale_factor)) => {
                            scale_factor.into_inner()
                        }
                        _ => return Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
                    };
                    Ok(GrokFilter::Scale(scale_factor))
                }
                _ => Err(GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            },
            "integer" => Ok(GrokFilter::Integer),
            "integerExt" => Ok(GrokFilter::IntegerExt),
            "number" => Ok(GrokFilter::Number),
            "numberExt" => Ok(GrokFilter::NumberExt),
            "lowercase" => Ok(GrokFilter::Lowercase),
            "uppercase" => Ok(GrokFilter::Uppercase),
            "json" => Ok(GrokFilter::Json),
            "rubyhash" => Ok(GrokFilter::Rubyhash),
            "querystring" => Ok(GrokFilter::Querystring),
            "decodeuricomponent" => Ok(GrokFilter::Decodeuricomponent),
            "boolean" => Ok(GrokFilter::Boolean),
            "xml" => Ok(GrokFilter::Xml),
            "nullIf" => f
                .args
                .as_ref()
                .and_then(|args| args.first()?.to_utf8_lossy().map(GrokFilter::NullIf))
                .ok_or_else(|| GrokStaticError::InvalidFunctionArguments(f.name.clone())),
            "array" => array::filter_from_function(f),
            "keyvalue" => keyvalue::filter_from_function(f),
            _ => Err(GrokStaticError::UnknownFilter(f.name.clone())),
        }
    }
}

/// Applies a given Grok filter to the value and returns the result or error.
/// For detailed description and examples of specific filters check out <https://docs.datadoghq.com/logs/log_configuration/parsing/?tab=filters>
pub fn apply_filter(value: &Value, filter: &GrokFilter) -> Result<Value, InternalError> {
    match filter {
        GrokFilter::Integer
        | GrokFilter::IntegerExt
        | GrokFilter::Number
        | GrokFilter::NumberExt
        | GrokFilter::Scale(_) => apply_numeric_filter(value, filter),
        GrokFilter::Lowercase => apply_utf8_filter(value, filter, str::to_lowercase),
        GrokFilter::Uppercase => apply_utf8_filter(value, filter, str::to_uppercase),
        GrokFilter::Json => parse_value_error_prone(value, filter, |b| {
            serde_json::from_slice::<'_, serde_json::Value>(b)
        }),
        GrokFilter::Rubyhash => try_apply_utf8_filter(value, filter, parse_ruby_hash),
        GrokFilter::Querystring => {
            parse_value_error_prone(value, filter, |s| parse_query_string(s, true))
        }
        GrokFilter::Boolean => apply_utf8_filter(value, filter, |s| "true".eq_ignore_ascii_case(s)),
        GrokFilter::Decodeuricomponent => parse_value(value, filter, |b| {
            percent_decode(b).decode_utf8_lossy().to_string()
        }),
        GrokFilter::Xml => parse_value_error_prone(value, filter, |_b| {
            parse_xml(
                value.to_owned(),
                ParseOptions {
                    attr_prefix: Some("".into()),
                    parse_number: Some(false.into()),
                    parse_bool: Some(false.into()),
                    parse_null: Some(false.into()),
                    text_key: Some("value".into()),
                    ..Default::default()
                },
            )
        }),
        GrokFilter::NullIf(null_value) => match value.as_str() {
            Some(s) if s == *null_value => Ok(Value::Null),
            Some(_) => Ok(value.to_owned()),
            None => Err(InternalError::FailedToApplyFilter(
                filter.to_string(),
                value.to_string(),
            )),
        },
        GrokFilter::Date(date_filter) => apply_date_filter(value, date_filter),
        GrokFilter::KeyValue(keyvalue_filter) => keyvalue_filter.apply_filter(value),
        GrokFilter::Array(brackets, delimiter, value_filter) => match value.as_str() {
            Some(input) => array::parse(
                &input,
                brackets
                    .as_ref()
                    .map(|(start, end)| (start.as_str(), end.as_str())),
                delimiter.as_ref().map(std::string::String::as_str),
            )
            .map_err(|_e| InternalError::FailedToApplyFilter(filter.to_string(), value.to_string()))
            .and_then(|values| {
                if let Some(value_filter) = value_filter.as_ref() {
                    return values
                        .iter()
                        .map(|v| apply_filter(v, value_filter))
                        .collect::<Result<Vec<Value>, _>>()
                        .map(Value::from);
                }
                Ok(values.into())
            }),
            None => Err(InternalError::FailedToApplyFilter(
                filter.to_string(),
                value.to_string(),
            )),
        },
    }
}

fn apply_numeric_filter(value: &Value, filter: &GrokFilter) -> Result<Value, InternalError> {
    if let Some(s) = value.as_str() {
        return parse_numeric_str(&s, filter, value);
    }
    match (filter, value) {
        (GrokFilter::Scale(scale_factor), Value::Integer(int_value)) => {
            Ok(scale_value(i64_to_f64(*int_value), *scale_factor))
        }
        (GrokFilter::Scale(scale_factor), Value::Float(float_value)) => {
            Ok(scale_value(float_value.into_inner(), *scale_factor))
        }
        _ => Err(filter_error(filter, value)),
    }
}

fn parse_numeric_str(s: &str, filter: &GrokFilter, value: &Value) -> Result<Value, InternalError> {
    match filter {
        GrokFilter::Integer => s
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| filter_error(filter, value)),
        GrokFilter::IntegerExt => s
            .parse::<f64>()
            .map(|parsed| Value::Integer(f64_to_i64(parsed)))
            .map_err(|_| filter_error(filter, value)),
        GrokFilter::Number | GrokFilter::NumberExt => s
            .parse::<f64>()
            .map(number_value)
            .map_err(|_| filter_error(filter, value)),
        GrokFilter::Scale(scale_factor) => s
            .parse::<f64>()
            .map(|parsed| scale_value(parsed, *scale_factor))
            .map_err(|_| filter_error(filter, value)),
        _ => unreachable!("called only for numeric filters"),
    }
}

fn filter_error(filter: &GrokFilter, value: &Value) -> InternalError {
    InternalError::FailedToApplyFilter(filter.to_string(), value.to_string())
}

fn i64_to_f64(value: i64) -> f64 {
    Value::Integer(value)
        .try_into_f64()
        .expect("integer values can always be converted to f64")
}

fn f64_to_i64(value: f64) -> i64 {
    Value::from_f64_or_zero(value)
        .try_into_i64()
        .expect("float values can always be converted to i64")
}

pub(super) fn f64_to_i64_if_integral(value: f64) -> Option<i64> {
    let integer = f64_to_i64(value);
    matches!((i64_to_f64(integer) - value).classify(), FpCategory::Zero).then_some(integer)
}

fn number_value(value: f64) -> Value {
    let normalized = Value::from_f64_or_zero(value);
    let Value::Float(value) = &normalized else {
        return normalized;
    };

    f64_to_i64_if_integral(value.into_inner()).map_or(normalized, Value::Integer)
}

fn scale_value(value: f64, scale_factor: f64) -> Value {
    let scale_factor = scale_factor * 1000_f64 / 1000_f64;
    number_value(value * scale_factor)
}

fn apply_utf8_filter<V: Into<Value>>(
    value: &Value,
    filter: &GrokFilter,
    parse: impl Fn(&str) -> V,
) -> Result<Value, InternalError> {
    value
        .as_str()
        .map(|s| parse(&s).into())
        .ok_or_else(|| filter_error(filter, value))
}

fn try_apply_utf8_filter<V: Into<Value>, E: std::error::Error>(
    value: &Value,
    filter: &GrokFilter,
    parse: impl Fn(&str) -> Result<V, E>,
) -> Result<Value, InternalError> {
    value
        .as_str()
        .ok_or_else(|| filter_error(filter, value))
        .and_then(|s| {
            parse(&s)
                .map(Into::into)
                .map_err(|_e| filter_error(filter, value))
        })
}

fn parse_value<V: Into<Value>>(
    value: &Value,
    filter: &GrokFilter,
    parse: impl Fn(&Bytes) -> V,
) -> Result<Value, InternalError> {
    match value.as_bytes() {
        Some(bytes) => Ok(parse(bytes).into()),
        None => Err(InternalError::FailedToApplyFilter(
            filter.to_string(),
            value.to_string(),
        )),
    }
}

fn parse_value_error_prone<V: Into<Value>, E: std::error::Error>(
    value: &Value,
    filter: &GrokFilter,
    parse: impl Fn(&Bytes) -> Result<V, E>,
) -> Result<Value, InternalError> {
    match value.as_bytes() {
        Some(bytes) => parse(bytes)
            .map_err(|_e| InternalError::FailedToApplyFilter(filter.to_string(), value.to_string()))
            .map(Into::into),
        _ => Err(InternalError::FailedToApplyFilter(
            filter.to_string(),
            value.to_string(),
        )),
    }
}
