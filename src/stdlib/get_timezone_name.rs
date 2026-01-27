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
#[cfg_attr(feature = "__mock_return_values_for_tests", allow(dead_code))]
fn get_timezone_name(ctx: &mut Context) -> Resolved {
    Ok(get_name_for_timezone(ctx.timezone()).into())
}

#[derive(Clone, Copy, Debug)]
pub struct GetTimezoneName;

impl Function for GetTimezoneName {
    fn identifier(&self) -> &'static str {
        "get_timezone_name"
    }

    fn usage(&self) -> &'static str {
        indoc! {r#"
            Returns the name of the timezone in the Vector configuration (see
            [global configuration options](/docs/reference/configuration/global-options)).
            If the configuration is set to `local`, then it attempts to
            determine the name of the timezone from the host OS. If this
            is not possible, then it returns the fixed offset of the
            local timezone for the current time in the format `"[+-]HH:MM"`,
            for example, `"+02:00"`.
        "#}
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["Retrieval of local timezone information failed."]
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get the IANA name of Vector's timezone",
            source: r#"get_timezone_name!() != """#,
            result: Ok("true"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get the IANA name of Vector's timezone",
            source: r#"get_timezone_name!()"#,
            result: Ok("UTC"),
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
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        get_timezone_name(ctx)
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, _ctx: &mut Context) -> Resolved {
        Ok("UTC".into())
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
