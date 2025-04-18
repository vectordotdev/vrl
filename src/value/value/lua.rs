use mlua::prelude::LuaResult;
use mlua::{FromLua, IntoLua, Lua, Value as LuaValue};
use ordered_float::NotNan;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::value::{KeyString, Value};

impl IntoLua for Value {
    #![allow(clippy::wrong_self_convention)] // this trait is defined by mlua
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Self::Bytes(b) => lua.create_string(b.as_ref()).map(LuaValue::String),
            Self::Regex(regex) => lua
                .create_string(regex.as_bytes_slice())
                .map(LuaValue::String),
            Self::Integer(i) => Ok(LuaValue::Integer(i)),
            Self::Float(f) => Ok(LuaValue::Number(f.into_inner())),
            Self::Boolean(b) => Ok(LuaValue::Boolean(b)),
            Self::Timestamp(t) => timestamp_to_table(lua, t).map(LuaValue::Table),
            Self::Object(m) => lua.create_table_from(m).map(LuaValue::Table),
            Self::Array(a) => lua.create_sequence_from(a).map(LuaValue::Table),
            Self::Null => lua.create_string("").map(LuaValue::String),
        }
    }
}

impl FromLua for Value {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::String(s) => Ok(Self::Bytes(s.as_bytes().to_vec().into())),
            LuaValue::Integer(i) => Ok(Self::Integer(i)),
            LuaValue::Number(f) => {
                let f = NotNan::new(f).map_err(|_| mlua::Error::FromLuaConversionError {
                    from: value.type_name(),
                    to: String::from("Value"),
                    message: Some("NaN not supported".to_string()),
                })?;
                Ok(Self::Float(f))
            }
            LuaValue::Boolean(b) => Ok(Self::Boolean(b)),
            LuaValue::Table(t) => {
                if t.len()? > 0 {
                    <_>::from_lua(LuaValue::Table(t), lua).map(|v: Vec<Self>| Self::Array(v.into()))
                } else if table_is_timestamp(&t)? {
                    table_to_timestamp(t).map(Self::Timestamp)
                } else {
                    <_>::from_lua(LuaValue::Table(t), lua)
                        .map(|v: BTreeMap<KeyString, Self>| Self::Object(v.into()))
                }
            }
            other => Err(mlua::Error::FromLuaConversionError {
                from: other.type_name(),
                to: String::from("Value"),
                message: Some("Unsupported Lua type".to_string()),
            }),
        }
    }
}

impl FromLua for ObjectMap {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        Ok(Self(Arc::new(BTreeMap::from_lua(value, lua)?)))
    }
}

use crate::prelude::ObjectMap;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use mlua::prelude::*;

/// Convert a `DateTime<Utc>` to a `LuaTable`.
///
/// # Errors
///
/// This function will fail insertion into the table fails.
pub fn timestamp_to_table(lua: &Lua, ts: DateTime<Utc>) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.raw_set("year", ts.year())?;
    table.raw_set("month", ts.month())?;
    table.raw_set("day", ts.day())?;
    table.raw_set("hour", ts.hour())?;
    table.raw_set("min", ts.minute())?;
    table.raw_set("sec", ts.second())?;
    table.raw_set("nanosec", ts.nanosecond())?;
    table.raw_set("yday", ts.ordinal())?;
    table.raw_set("wday", ts.weekday().number_from_sunday())?;
    table.raw_set("isdst", false)?;

    Ok(table)
}

/// Determines if a `LuaTable` is a timestamp.
///
/// # Errors
///
/// This function will fail if the table is malformed.
pub fn table_is_timestamp(t: &LuaTable) -> LuaResult<bool> {
    for &key in &["year", "month", "day", "hour", "min", "sec"] {
        if !t.contains_key(key)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Convert a `LuaTable` to a `DateTime<Utc>`.
///
/// # Errors
///
/// This function will fail if the table is malformed.
#[allow(clippy::needless_pass_by_value)] // constrained by mlua types
pub fn table_to_timestamp(t: LuaTable) -> LuaResult<DateTime<Utc>> {
    let year = t.raw_get("year")?;
    let month = t.raw_get("month")?;
    let day = t.raw_get("day")?;
    let hour = t.raw_get("hour")?;
    let min = t.raw_get("min")?;
    let sec = t.raw_get("sec")?;
    let nano = t.raw_get::<Option<u32>>("nanosec")?.unwrap_or(0);

    let base_dt = Utc
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
        .ok_or_else(|| mlua::Error::external("invalid timestamp"))?;

    if nano > 0 {
        base_dt
            .with_nanosecond(nano)
            .ok_or_else(|| mlua::Error::external("could not adjust for nanoseconds"))
    } else {
        Ok(base_dt)
    }
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn from_lua() {
        let pairs = vec![
            (
                "'\u{237a}\u{3b2}\u{3b3}'",
                Value::Bytes("\u{237a}\u{3b2}\u{3b3}".into()),
            ),
            ("123", Value::Integer(123)),
            ("4.333", Value::from(4.333)),
            ("true", Value::Boolean(true)),
            (
                "{ x = 1, y = '2', nested = { other = 5.678 } }",
                Value::Object(
                    vec![
                        ("x".into(), 1_i64.into()),
                        ("y".into(), "2".into()),
                        (
                            "nested".into(),
                            Value::Object(
                                vec![("other".into(), 5.678.into())].into_iter().collect(),
                            ),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ),
            (
                "{1, '2', 0.57721566}",
                Value::Array(vec![1_i64.into(), "2".into(), 0.577_215_66.into()].into()),
            ),
            (
                "os.date('!*t', 1584297428)",
                Value::Timestamp(
                    Utc.with_ymd_and_hms(2020, 3, 15, 18, 37, 8)
                        .single()
                        .expect("invalid or ambiguous date and time"),
                ),
            ),
            (
                "{year=2020, month=3, day=15, hour=18, min=37, sec=8}",
                Value::Timestamp(
                    Utc.with_ymd_and_hms(2020, 3, 15, 18, 37, 8)
                        .single()
                        .expect("invalid or ambiguous date and time"),
                ),
            ),
            (
                "{year=2020, month=3, day=15, hour=18, min=37, sec=8, nanosec=666666666}",
                Value::Timestamp(
                    Utc.with_ymd_and_hms(2020, 3, 15, 18, 37, 8)
                        .single()
                        .expect("invalid or ambiguous date and time")
                        .with_nanosecond(666_666_666)
                        .expect("invalid nanosecond"),
                ),
            ),
        ];

        let lua = Lua::new();
        for (expression, expected) in pairs {
            let value: Value = lua.load(expression).eval().unwrap();
            assert_eq!(value, expected, "expression: {expression:?}");
        }
    }

    #[test]
    // Long test is long.
    #[allow(clippy::too_many_lines)]
    fn to_lua() {
        let pairs = vec![
            (
                Value::Bytes("\u{237a}\u{3b2}\u{3b3}".into()),
                r"
                function (value)
                    return value == '\u{237a}\u{3b2}\u{3b3}'
                end
                ",
            ),
            (
                Value::Integer(123),
                "
                function (value)
                    return value == 123
                end
                ",
            ),
            (
                Value::from(4.333),
                "
                function (value)
                    return value == 4.333
                end
                ",
            ),
            (
                Value::Null,
                "
                function (value)
                    return value == ''
                end
                ",
            ),
            (
                Value::Object(
                    vec![
                        ("x".into(), 1_i64.into()),
                        ("y".into(), "2".into()),
                        (
                            "nested".into(),
                            Value::Object(
                                vec![("other".into(), 5.111.into())].into_iter().collect(),
                            ),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                "
                function (value)
                    return value.x == 1 and
                        value['y'] == '2' and
                        value.nested.other == 5.111
                end
                ",
            ),
            (
                Value::Array(vec![1_i64.into(), "2".into(), 0.577_215_66.into()].into()),
                "
                function (value)
                    return value[1] == 1 and
                        value[2] == '2' and
                        value[3] == 0.57721566
                end
                ",
            ),
            (
                Value::Timestamp(
                    Utc.with_ymd_and_hms(2020, 3, 15, 18, 37, 8)
                        .single()
                        .expect("invalid or ambiguous date and time")
                        .with_nanosecond(666_666_666)
                        .expect("invalid nanosecond"),
                ),
                r#"
                function (value)
                    local expected = os.date("!*t", 1584297428)
                    expected.nanosec = 666666666

                    return os.time(value) == os.time(expected) and
                        value.nanosec == expected.nanosec and
                        value.yday == expected.yday and
                        value.wday == expected.wday and
                        value.isdst == expected.isdst
                end
                "#,
            ),
        ];

        let lua = Lua::new();
        for (value, test_src) in pairs {
            let test_fn: LuaFunction = lua
                .load(test_src)
                .eval()
                .unwrap_or_else(|_| panic!("Failed to load {test_src} for value {value:?}"));
            assert!(
                test_fn
                    .call::<bool>(value.clone())
                    .unwrap_or_else(|_| panic!("Failed to call {test_src} for value {value:?}")),
                "Test function: {test_src}, value: {value:?}"
            );
        }
    }
}
