use mlua::prelude::LuaResult;
use mlua::{FromLua, IntoLua, Lua, Value as LuaValue};
use ordered_float::NotNan;

use std::collections::BTreeMap;

use crate::value::{KeyString, ObjectMap, Value};

impl FromLua for ObjectMap {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        <BTreeMap<KeyString, Value>>::from_lua(value, lua).map(Self::from)
    }
}

impl IntoLua for Value {
    #![allow(clippy::wrong_self_convention)] // this trait is defined by mlua
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Self::Bytes(b) => lua.create_string(b.as_ref()).map(LuaValue::String),
            Self::String(s) => lua.create_string(&*s).map(LuaValue::String),
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
            LuaValue::String(s) => Ok(Self::from_utf8_or_bytes(s.as_bytes().to_vec().into())),
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
                    <_>::from_lua(LuaValue::Table(t), lua).map(Self::Array)
                } else if table_is_timestamp(&t)? {
                    table_to_timestamp(t).map(Self::Timestamp)
                } else {
                    ObjectMap::from_lua(LuaValue::Table(t), lua).map(Self::Object)
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

use jiff::{Timestamp, civil, tz::Offset};
use mlua::prelude::*;

/// Convert a `Timestamp` to a `LuaTable`, in UTC.
///
/// # Errors
///
/// This function will fail insertion into the table fails.
pub fn timestamp_to_table(lua: &Lua, ts: Timestamp) -> LuaResult<LuaTable> {
    let dt = Offset::UTC.to_datetime(ts);
    let table = lua.create_table()?;
    table.raw_set("year", dt.year())?;
    table.raw_set("month", dt.month())?;
    table.raw_set("day", dt.day())?;
    table.raw_set("hour", dt.hour())?;
    table.raw_set("min", dt.minute())?;
    table.raw_set("sec", dt.second())?;
    table.raw_set("nanosec", dt.subsec_nanosecond())?;
    table.raw_set("yday", dt.day_of_year())?;
    table.raw_set("wday", dt.weekday().to_sunday_one_offset())?;
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

/// An integer read from Lua that truncates fractional numbers toward zero.
///
/// mlua 0.12 rejects non-integral numbers when converting to Rust integer types under Lua 5.4,
/// while mlua 0.11 truncated them. Scripts commonly compute timestamp fields with float
/// arithmetic (for example `nanosec = 1e9 / 3`), so this wrapper keeps the previous truncating
/// behavior. Out-of-range and non-numeric values still fail the conversion.
struct LuaInt<T>(T);

impl<T: FromLua> FromLua for LuaInt<T> {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let value = match value {
            LuaValue::Number(n) => LuaValue::Number(n.trunc()),
            LuaValue::String(_) => {
                if let Some(i) = lua.coerce_integer(value.clone())? {
                    LuaValue::Integer(i)
                } else if let Some(n) = lua.coerce_number(value.clone())? {
                    LuaValue::Number(n.trunc())
                } else {
                    value
                }
            }
            other => other,
        };
        T::from_lua(value, lua).map(Self)
    }
}

/// Convert a `LuaTable` in UTC to a `Timestamp`.
///
/// # Errors
///
/// This function will fail if the table is malformed.
#[allow(clippy::needless_pass_by_value)] // constrained by mlua types
pub fn table_to_timestamp(t: LuaTable) -> LuaResult<Timestamp> {
    let LuaInt(year) = t.raw_get("year")?;
    let LuaInt(month) = t.raw_get("month")?;
    let LuaInt(day) = t.raw_get("day")?;
    let LuaInt(hour) = t.raw_get("hour")?;
    let LuaInt(min) = t.raw_get("min")?;
    let LuaInt(sec) = t.raw_get("sec")?;
    let nano = t
        .raw_get::<Option<LuaInt<i32>>>("nanosec")?
        .map_or(0, |LuaInt(nano)| nano);
    let date = civil::Date::new(year, month, day)
        .map_err(|_| mlua::Error::external("invalid timestamp"))?;
    let time = civil::Time::new(hour, min, sec, 0)
        .map_err(|_| mlua::Error::external("invalid timestamp"))?
        .with()
        .subsec_nanosecond(nano)
        .build()
        .map_err(|_| mlua::Error::external("could not adjust for nanoseconds"))?;

    Offset::UTC
        .to_timestamp(date.to_datetime(time))
        .map_err(|_| mlua::Error::external("invalid timestamp"))
}

#[cfg(test)]
mod test {
    use super::*;

    fn utc(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

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
                "{1, '2', 0.56721566}",
                Value::Array(vec![1_i64.into(), "2".into(), 0.567_215_66.into()]),
            ),
            (
                "os.date('!*t', 1584297428)",
                Value::Timestamp(utc("2020-03-15T18:37:08Z")),
            ),
            (
                "{year=2020, month=3, day=15, hour=18, min=37, sec=8}",
                Value::Timestamp(utc("2020-03-15T18:37:08Z")),
            ),
            (
                "{year=2020, month=3, day=15, hour=18, min=37, sec=8, nanosec=666666666}",
                Value::Timestamp(utc("2020-03-15T18:37:08.666666666Z")),
            ),
        ];

        let lua = Lua::new();
        for (expression, expected) in pairs {
            let value: Value = lua.load(expression).eval().unwrap();
            assert_eq!(value, expected, "expression: {expression:?}");
        }
    }

    #[test]
    fn from_lua_timestamp_truncates_fractional_fields() {
        let expected = Value::Timestamp(utc("2020-03-15T18:37:08.333333333Z"));

        let lua = Lua::new();
        for expression in [
            "{year=2020, month=3, day=15, hour=18, min=37, sec=8, nanosec=1e9 / 3}",
            "{year=2020, month=3, day=15, hour=18, min=37, sec=8.9, nanosec='333333333.9'}",
            "(function() local t = os.date('!*t', 1584297428); t.nanosec = 1e9 / 3; return t end)()",
        ] {
            let value: Value = lua.load(expression).eval().unwrap();
            assert_eq!(value, expected, "expression: {expression:?}");
        }
    }

    #[test]
    fn from_lua_timestamp_rejects_out_of_range_fields() {
        let lua = Lua::new();
        let result: LuaResult<Value> = lua
            .load("{year=2020, month=3, day=15, hour=18, min=37, sec=8, nanosec=-1.5}")
            .eval();
        assert!(result.is_err());
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
                Value::Array(vec![1_i64.into(), "2".into(), 0.567_215_66.into()]),
                "
                function (value)
                    return value[1] == 1 and
                        value[2] == '2' and
                        value[3] == 0.56721566
                end
                ",
            ),
            (
                Value::Timestamp(utc("2020-03-15T18:37:08.666666666Z")),
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
