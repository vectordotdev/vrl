use crate::compiler::prelude::*;
use chrono::Utc;

#[derive(Clone, Copy, Debug)]
pub struct Now;

impl Function for Now {
    fn identifier(&self) -> &'static str {
        "now"
    }

    fn usage(&self) -> &'static str {
        "Returns the current timestamp in the UTC timezone with nanosecond precision."
    }

    fn category(&self) -> &'static str {
        Category::Timestamp.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::TIMESTAMP
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "now",
            source: r#"now() != """#,
            result: Ok("true"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Generate a current timestamp",
            source: r#"now()"#,
            result: Ok("2012-03-04T12:34:56.789012345Z"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(NowFn.as_expr())
    }
}

#[derive(Debug, Clone)]
struct NowFn;

impl FunctionExpression for NowFn {
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(Utc::now().into())
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, _: &mut Context) -> Resolved {
        use chrono::{NaiveDate, TimeZone};

        let d = NaiveDate::from_ymd_opt(2012, 3, 4).unwrap();
        let d = d.and_hms_nano_opt(12, 34, 56, 789_012_345).unwrap();

        Ok(Utc.from_local_datetime(&d).unwrap().into())
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::timestamp()
    }
}
