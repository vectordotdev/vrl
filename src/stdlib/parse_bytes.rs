use crate::compiler::prelude::*;
use crate::value;
use once_cell::sync::Lazy;
use regex::Regex;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::{collections::HashMap, str::FromStr};

fn parse_bytes(bytes: Value, unit: Value, base: &Bytes) -> Resolved {
    let units = match base.as_ref() {
        b"2" => &*BIN_UNITS,
        b"10" => &*DEC_UNITS,
        _ => unreachable!("enum invariant"),
    };
    let bytes = bytes.try_bytes()?;
    let value = String::from_utf8_lossy(&bytes);
    let conversion_factor = {
        let bytes = unit.try_bytes()?;
        let string = String::from_utf8_lossy(&bytes);

        units
            .get(string.as_ref())
            .ok_or(format!("unknown unit format: '{string}'"))?
    };
    let captures = RE
        .captures(&value)
        .ok_or(format!("unable to parse duration: '{value}'"))?;
    let value = Decimal::from_str(&captures["value"])
        .map_err(|error| format!("unable to parse number: {error}"))?;
    let unit = units
        .get(&captures["unit"])
        .ok_or(format!("unknown duration unit: '{}'", &captures["unit"]))?;
    let number = value * unit / conversion_factor;
    let number = number
        .to_f64()
        .ok_or(format!("unable to format duration: '{number}'"))?;
    Ok(Value::from_f64_or_zero(number))
}

static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)                        # i: case-insensitive, x: ignore whitespace + comments
            \A
            (?P<value>[0-9]*\.?[0-9]+) # value: integer or float
            \s?                        # optional space between value and unit
            (?P<unit>[a-z]{1,3})      # unit: one or two letters
            \z",
    )
    .unwrap()
});
// The largest unit is EB, which is smaller than i64::MAX, so we can safely use Decimal
// power of 2 units
static BIN_UNITS: Lazy<HashMap<String, Decimal>> = Lazy::new(|| {
    vec![
        ("B", Decimal::new(1, 0)),
        ("KiB", Decimal::new(1_024, 0)),
        ("MiB", Decimal::new(1_048_576, 0)),
        ("GiB", Decimal::new(1_073_741_824, 0)),
        ("TiB", Decimal::new(1_099_511_627_776, 0)),
        ("PiB", Decimal::new(1_125_899_906_842_624, 0)),
        ("EiB", Decimal::new(1_152_921_504_606_846_976, 0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
});
// power of 10 units
static DEC_UNITS: Lazy<HashMap<String, Decimal>> = Lazy::new(|| {
    vec![
        ("B", Decimal::new(1, 0)),
        ("kB", Decimal::new(1_000, 0)),
        ("MB", Decimal::new(1_000_000, 0)),
        ("GB", Decimal::new(1_000_000_000, 0)),
        ("TB", Decimal::new(1_000_000_000_000, 0)),
        ("PB", Decimal::new(1_000_000_000_000_000, 0)),
        ("EB", Decimal::new(1_000_000_000_000_000_000, 0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
});

#[derive(Clone, Copy, Debug)]
pub struct ParseBytes;

fn base_sets() -> Vec<Value> {
    vec![value!("2"), value!("10")]
}

impl Function for ParseBytes {
    fn identifier(&self) -> &'static str {
        "parse_bytes"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "milliseconds",
            source: r#"parse_bytes!("1GB", unit: "B", base: "10")"#,
            result: Ok("1_000_000_000"),
        }]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let unit = arguments.required("unit");
        let base = arguments
            .optional_enum("base", &base_sets(), state)?
            .unwrap_or_else(|| value!("2"))
            .try_bytes()
            .expect("base not bytes");

        Ok(ParseBytesFn { value, unit, base }.as_expr())
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
            Parameter {
                keyword: "base",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ParseBytesFn {
    value: Box<dyn Expression>,
    unit: Box<dyn Expression>,
    base: Bytes,
}

impl FunctionExpression for ParseBytesFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.value.resolve(ctx)?;
        let unit = self.unit.resolve(ctx)?;

        parse_bytes(bytes, unit, &self.base)
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
        parse_bytes => ParseBytes;

        mib_b {
            args: func_args![value: "1MiB",
                             unit: "B"],
            want: Ok(value!(1_048_576.0)),
            tdef: TypeDef::float().fallible(),
        }

        b_kib {
            args: func_args![value: "512B",
                             unit: "KiB"],
            want: Ok(0.5),
            tdef: TypeDef::float().fallible(),
        }

        gib_mib {
            args: func_args![value: "3.5GiB",
                             unit: "KiB"],
            want: Ok(3_670_016.0),
            tdef: TypeDef::float().fallible(),
        }

        tib_gib {
            args: func_args![value: "12 TiB",
                             unit: "GiB"],
            want: Ok(12_288.0),
            tdef: TypeDef::float().fallible(),
        }

        mib_pib {
            args: func_args![value: "256TiB",
                             unit: "PiB"],
            want: Ok(0.25),
            tdef: TypeDef::float().fallible(),
        }

        eib_tib {
            args: func_args![value: "1EiB",
                             unit: "TiB"],
            want: Ok(value!(1_048_576.0)),
            tdef: TypeDef::float().fallible(),
        }

        mb_b {
            args: func_args![value: "1MB",
                             unit: "B",
                             base: "10"],
            want: Ok(value!(1_000_000.0)),
            tdef: TypeDef::float().fallible(),
        }

        b_kb {
            args: func_args![value: "3B",
                             unit: "kB",
                             base: "10"],
            want: Ok(0.003),
            tdef: TypeDef::float().fallible(),
        }

        gb_mb {
            args: func_args![value: "3.007GB",
                             unit: "kB",
                             base: "10"],
            want: Ok(3_007_000.0),
            tdef: TypeDef::float().fallible(),
        }

        tb_gb {
            args: func_args![value: "12 TB",
                             unit: "GB",
                             base: "10"],
            want: Ok(12_000.0),
            tdef: TypeDef::float().fallible(),
        }

        mb_pb {
            args: func_args![value: "768MB",
                             unit: "PB",
                             base: "10"],
            want: Ok(0.000000768),
            tdef: TypeDef::float().fallible(),
        }

        eb_tb {
            args: func_args![value: "1EB",
                             unit: "TB",
                             base: "10"],
            want: Ok(value!(1_000_000.0)),
            tdef: TypeDef::float().fallible(),
        }

        error_invalid {
            args: func_args![value: "foo",
                             unit: "KiB"],
            want: Err("unable to parse duration: 'foo'"),
            tdef: TypeDef::float().fallible(),
        }

        error_kb {
            args: func_args![value: "1",
                             unit: "KiB"],
            want: Err("unable to parse duration: '1'"),
            tdef: TypeDef::float().fallible(),
        }

        error_unit {
            args: func_args![value: "1YiB",
                             unit: "MiB"],
            want: Err("unknown duration unit: 'YiB'"),
            tdef: TypeDef::float().fallible(),
        }

        error_format {
            args: func_args![value: "100KiB",
                             unit: "ZB",
                             base: "10"],
            want: Err("unknown unit format: 'ZB'"),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
