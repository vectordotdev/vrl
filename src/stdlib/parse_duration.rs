use crate::compiler::prelude::*;
use humantime::parse_duration as ht_parse_duration;
use once_cell::sync::Lazy;
use regex::Regex;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::time::Duration;
use std::{collections::HashMap, str::FromStr};
use tracing::warn;

fn parse_duration(bytes: Value, unit: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let value = String::from_utf8_lossy(&bytes);

    // Remove all spaces and replace the micro symbol with the ASCII equivalent
    // since the `humantime` does not support them.
    let trimmed_value = value.replace(' ', "").replace("µs", "us");

    // Parse the conversion factor
    let conversion_factor = {
        let bytes = unit.clone().try_bytes()?;
        let string = String::from_utf8_lossy(&bytes);

        *DURATION_UNITS
            .get(string.as_ref())
            .ok_or(format!("unknown unit format: '{string}'"))?
    };

    // Try the `ht_parse_duration` first
    match ht_parse_duration(&trimmed_value) {
        Ok(duration) => {
            let number = duration.div_duration_f64(conversion_factor);
            Ok(Value::from_f64_or_zero(number))
        }
        Err(ht_error) => {
            warn!(message = "parsing duration with humantime failed, falling back to regex", trimmed_value = %trimmed_value,error =  %ht_error);
            parse_duration_regex(&value, unit)
        }
    }
}

fn parse_duration_regex(value: &str, unit: Value) -> Resolved {
    let mut value = &value[..];
    let conversion_factor = {
        let bytes = unit.try_bytes()?;
        let string = String::from_utf8_lossy(&bytes);

        DECIMAL_UNITS
            .get(string.as_ref())
            .ok_or(format!("unknown unit format: '{string}'"))?
    };
    let mut num = 0.0;
    while !value.is_empty() {
        let captures = RE
            .captures(value)
            .ok_or(format!("unable to parse duration: '{value}'"))?;
        let capture_match = captures.get(0).unwrap();

        let value_decimal = Decimal::from_str(&captures["value"])
            .map_err(|error| format!("unable to parse number: {error}"))?;
        let unit = DECIMAL_UNITS
            .get(&captures["unit"])
            .ok_or(format!("unknown duration unit: '{}'", &captures["unit"]))?;
        let number = value_decimal * unit / conversion_factor;
        let number = number
            .to_f64()
            .ok_or(format!("unable to format duration: '{number}'"))?;
        num += number;
        value = &value[capture_match.end()..];
    }
    Ok(Value::from_f64_or_zero(num))
}

static DURATION_UNITS: Lazy<HashMap<String, Duration>> = Lazy::new(|| {
    vec![
        ("ns", Duration::from_nanos(1)),
        ("us", Duration::from_micros(1)),
        ("µs", Duration::from_micros(1)),
        ("ms", Duration::from_millis(1)),
        ("cs", Duration::from_millis(10)),
        ("ds", Duration::from_millis(100)),
        ("s", Duration::from_secs(1)),
        ("m", Duration::from_secs(60)),
        ("h", Duration::from_secs(3_600)),
        ("d", Duration::from_secs(86_400)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
});

static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)                        # i: case-insensitive, x: ignore whitespace + comments
            \A
            (?P<value>[0-9]*\.?[0-9]+) # value: integer or float
            \s?                        # optional space between value and unit
            (?P<unit>[µa-z]{1,2})      # unit: one or two letters
            \z",
    )
    .unwrap()
});

static DECIMAL_UNITS: Lazy<HashMap<String, Decimal>> = Lazy::new(|| {
    vec![
        ("ns", Decimal::new(1, 9)),
        ("us", Decimal::new(1, 6)),
        ("µs", Decimal::new(1, 6)),
        ("ms", Decimal::new(1, 3)),
        ("cs", Decimal::new(1, 2)),
        ("ds", Decimal::new(1, 1)),
        ("s", Decimal::new(1, 0)),
        ("m", Decimal::new(60, 0)),
        ("h", Decimal::new(3_600, 0)),
        ("d", Decimal::new(86_400, 0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
});

#[derive(Clone, Copy, Debug)]
pub struct ParseDuration;

impl Function for ParseDuration {
    fn identifier(&self) -> &'static str {
        "parse_duration"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "milliseconds",
            source: r#"parse_duration!("1005ms", unit: "s")"#,
            result: Ok("1.005"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let unit = arguments.required("unit");

        Ok(ParseDurationFn { value, unit }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "unit",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ParseDurationFn {
    value: Box<dyn Expression>,
    unit: Box<dyn Expression>,
}

impl FunctionExpression for ParseDurationFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.value.resolve(ctx)?;
        let unit = self.unit.resolve(ctx)?;

        parse_duration(bytes, unit)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::float().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        parse_duration => ParseDuration;

        s_m {
            args: func_args![value: "30s",
                             unit: "m"],
            want: Ok(value!(0.5)),
            tdef: TypeDef::float().fallible(),
        }

        ms_ms {
            args: func_args![value: "100ms",
                             unit: "ms"],
            want: Ok(100.0),
            tdef: TypeDef::float().fallible(),
        }

        ms_s {
            args: func_args![value: "1005ms",
                             unit: "s"],
            want: Ok(1.005),
            tdef: TypeDef::float().fallible(),
        }

        ns_ms {
            args: func_args![value: "100ns",
                             unit: "ms"],
            want: Ok(0.0001),
            tdef: TypeDef::float().fallible(),
        }

        us_ms {
            args: func_args![value: "100µs",
                             unit: "ms"],
            want: Ok(0.1),
            tdef: TypeDef::float().fallible(),
        }

        d_s {
            args: func_args![value: "1d",
                             unit: "s"],
            want: Ok(86400.0),
            tdef: TypeDef::float().fallible(),
        }

        ds_s {
            args: func_args![value: "1d1s",
                             unit: "s"],
            want: Ok(86401.0),
            tdef: TypeDef::float().fallible(),
        }

        s_space_ms_ms {
            args: func_args![value: "1s 1ms",
                             unit: "ms"],
            want: Ok(1001.0),
            tdef: TypeDef::float().fallible(),
        }

        ms_space_us_ms {
            args: func_args![value: "1ms1 µs",
                             unit: "ms"],
            want: Ok(1.001),
            tdef: TypeDef::float().fallible(),
        }

        s_space_m_ms_order_agnostic {
            args: func_args![value: "1s1m",
                             unit: "ms"],
            want: Ok(61000.0),
            tdef: TypeDef::float().fallible(),
        }

        s_ns {
            args: func_args![value: "1 s",
                             unit: "ns"],
            want: Ok(1_000_000_000.0),
            tdef: TypeDef::float().fallible(),
        }

        us_space_ms {
            args: func_args![value: "1 µs",
                             unit: "ms"],
            want: Ok(0.001),
            tdef: TypeDef::float().fallible(),
        }

        w_ns {
            args: func_args![value: "1w",
                             unit: "ns"],
            want: Ok(604_800_000_000_000.0),
            tdef: TypeDef::float().fallible(),
        }

        decimal_s_ms {
            args: func_args![value: "12.3s",
                             unit: "ms"],
            want: Ok(12300.0),
            tdef: TypeDef::float().fallible(),
        }

        decimal_s_ms_2 {
            args: func_args![value: "123.0s",
                             unit: "ms"],
            want: Ok(123_000.0),
            tdef: TypeDef::float().fallible(),
        }

        error_invalid {
            args: func_args![value: "foo",
                             unit: "ms"],
            want: Err("unable to parse duration: 'foo'"),
            tdef: TypeDef::float().fallible(),
        }

        error_ns {
            args: func_args![value: "1",
                             unit: "ns"],
            want: Err("unable to parse duration: '1'"),
            tdef: TypeDef::float().fallible(),
        }

        error_format {
            args: func_args![value: "1s",
                             unit: "w"],
            want: Err("unknown unit format: 'w'"),
            tdef: TypeDef::float().fallible(),
        }

        error_failed_2nd_unit {
            args: func_args![value: "1d foo",
                             unit: "s"],
            want: Err("unable to parse duration: '1d foo'"),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
