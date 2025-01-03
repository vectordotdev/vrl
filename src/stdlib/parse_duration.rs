use crate::compiler::prelude::*;
use humantime::parse_duration as ht_parse_duration;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::time::Duration;

fn parse_duration(bytes: Value, unit: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let value = String::from_utf8_lossy(&bytes);
    // Remove all spaces and replace the micro symbol with the ASCII equivalent
    // since the `humantime` does not support them.
    let trimmed_value = value.replace(' ', "").replace("µs", "us");

    let conversion_factor = {
        let bytes = unit.try_bytes()?;
        let string = String::from_utf8_lossy(&bytes);

        *UNITS
            .get(string.as_ref())
            .ok_or(format!("unknown unit format: '{string}'"))?
    };
    let duration = ht_parse_duration(&trimmed_value)
        .map_err(|e| format!("unable to parse duration: '{e}'"))?;
    let number = duration.div_duration_f64(conversion_factor);

    Ok(Value::from_f64_or_zero(number))
}

static UNITS: Lazy<HashMap<String, Duration>> = Lazy::new(|| {
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

        error_invalid {
            args: func_args![value: "foo",
                             unit: "ms"],
            want: Err("unable to parse duration: 'expected number at 0'"),
            tdef: TypeDef::float().fallible(),
        }

        error_ns {
            args: func_args![value: "1",
                             unit: "ns"],
            want: Err("unable to parse duration: 'time unit needed, for example 1sec or 1ms'"),
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
            want: Err("unable to parse duration: 'unknown time unit \"dfoo\", supported units: ns, us, ms, sec, min, hours, days, weeks, months, years (and few variations)'"),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
