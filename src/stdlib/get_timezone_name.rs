use crate::compiler::prelude::*;
use chrono::Local;
use std::borrow::Cow;

#[must_use]
pub fn get_name_for_timezone(tz: &TimeZone) -> Cow<'_, str> {
    match tz {
        TimeZone::Named(tz) => tz.name().into(),
        TimeZone::Local => iana_time_zone::get_timezone()
            .unwrap_or_else(|_| Local::now().offset().to_string())
            .into(),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn get_timezone_name(ctx: &mut Context) -> Resolved {
    Ok(get_name_for_timezone(ctx.timezone()).into())
}

#[derive(Clone, Copy, Debug)]
pub struct GetTimezoneName;

impl Function for GetTimezoneName {
    fn identifier(&self) -> &'static str {
        "get_timezone_name"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "Get the VRL timezone name, or for 'local' the local timezone name or offset (e.g., -05:00)",
            source: r#"get_timezone_name!() != """#,
            result: Ok("true"),
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(GetTimezoneNameFn.as_expr())
    }
}

#[derive(Debug, Clone)]
struct GetTimezoneNameFn;

impl FunctionExpression for GetTimezoneNameFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        get_timezone_name(ctx)
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        get_hostname => GetTimezoneName;

        // the test harness always initializes the VRL timezone to UTC
        utc {
            args: func_args![],
            want: Ok(value!(get_name_for_timezone(&TimeZone::Named(chrono_tz::Tz::UTC)))),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
