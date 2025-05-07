use crate::compiler::prelude::*;

fn to_syslog_facility_code(facility: &Value) -> Resolved {
    let facility = facility.try_bytes_utf8_lossy()?;
    // Facility codes: https://en.wikipedia.org/wiki/Syslog#Facility
    let code = match &facility[..] {
        "kern" => 0,
        "user" => 1,
        "mail" => 2,
        "daemon" => 3,
        "auth" => 4,
        "syslog" => 5,
        "lpr" => 6,
        "news" => 7,
        "uucp" => 8,
        "cron" => 9,
        "authpriv" => 10,
        "ftp" => 11,
        "ntp" => 12,
        "security" => 13,
        "console" => 14,
        "solaris-cron" => 15,
        "local0" => 16,
        "local1" => 17,
        "local2" => 18,
        "local3" => 19,
        "local4" => 20,
        "local5" => 21,
        "local6" => 22,
        "local7" => 23,
        _ => return Err(format!("syslog facility '{facility}' not valid").into()),
    };
    Ok(code.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ToSyslogFacilityCode;

impl Function for ToSyslogFacilityCode {
    fn identifier(&self) -> &'static str {
        "to_syslog_facility_code"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid",
                source: "to_syslog_facility_code!(s'kern')",
                result: Ok("0"),
            },
            Example {
                title: "invalid",
                source: "to_syslog_facility_code!(s'foobar')",
                result: Err(
                    r#"function call error for "to_syslog_facility_code" at (0:35): syslog facility 'foobar' not valid"#,
                ),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ToSyslogFacilityCodeFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToSyslogFacilityCodeFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToSyslogFacilityCodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let facility = self.value.resolve(ctx)?;
        to_syslog_facility_code(&facility)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        to_code => ToSyslogFacilityCode;

        kern {
            args: func_args![value: value!("kern")],
            want: Ok(value!(0)),
            tdef: TypeDef::integer().fallible(),
        }

        user {
            args: func_args![value: value!("user")],
            want: Ok(value!(1)),
            tdef: TypeDef::integer().fallible(),
        }

        mail {
            args: func_args![value: value!("mail")],
            want: Ok(value!(2)),
            tdef: TypeDef::integer().fallible(),
        }

        daemon {
            args: func_args![value: value!("daemon")],
            want: Ok(value!(3)),
            tdef: TypeDef::integer().fallible(),
        }

        auth {
            args: func_args![value: value!("auth")],
            want: Ok(value!(4)),
            tdef: TypeDef::integer().fallible(),
        }

        syslog {
            args: func_args![value: value!("syslog")],
            want: Ok(value!(5)),
            tdef: TypeDef::integer().fallible(),
        }

        lpr {
            args: func_args![value: value!("lpr")],
            want: Ok(value!(6)),
            tdef: TypeDef::integer().fallible(),
        }

        news {
            args: func_args![value: value!("news")],
            want: Ok(value!(7)),
            tdef: TypeDef::integer().fallible(),
        }

        uucp {
            args: func_args![value: value!("uucp")],
            want: Ok(value!(8)),
            tdef: TypeDef::integer().fallible(),
        }

        cron {
            args: func_args![value: value!("cron")],
            want: Ok(value!(9)),
            tdef: TypeDef::integer().fallible(),
        }

        authpriv {
            args: func_args![value: value!("authpriv")],
            want: Ok(value!(10)),
            tdef: TypeDef::integer().fallible(),
        }

        ftp {
            args: func_args![value: value!("ftp")],
            want: Ok(value!(11)),
            tdef: TypeDef::integer().fallible(),
        }

        ntp {
            args: func_args![value: value!("ntp")],
            want: Ok(value!(12)),
            tdef: TypeDef::integer().fallible(),
        }

        security {
            args: func_args![value: value!("security")],
            want: Ok(value!(13)),
            tdef: TypeDef::integer().fallible(),
        }

        console {
            args: func_args![value: value!("console")],
            want: Ok(value!(14)),
            tdef: TypeDef::integer().fallible(),
        }

        solaris_cron {
            args: func_args![value: value!("solaris-cron")],
            want: Ok(value!(15)),
            tdef: TypeDef::integer().fallible(),
        }

        local0 {
            args: func_args![value: value!("local0")],
            want: Ok(value!(16)),
            tdef: TypeDef::integer().fallible(),
        }

        local1 {
            args: func_args![value: value!("local1")],
            want: Ok(value!(17)),
            tdef: TypeDef::integer().fallible(),
        }

        local2 {
            args: func_args![value: value!("local2")],
            want: Ok(value!(18)),
            tdef: TypeDef::integer().fallible(),
        }

        local3 {
            args: func_args![value: value!("local3")],
            want: Ok(value!(19)),
            tdef: TypeDef::integer().fallible(),
        }

        local4 {
            args: func_args![value: value!("local4")],
            want: Ok(value!(20)),
            tdef: TypeDef::integer().fallible(),
        }

        local5 {
            args: func_args![value: value!("local5")],
            want: Ok(value!(21)),
            tdef: TypeDef::integer().fallible(),
        }

        local6 {
            args: func_args![value: value!("local6")],
            want: Ok(value!(22)),
            tdef: TypeDef::integer().fallible(),
        }

        local7 {
            args: func_args![value: value!("local7")],
            want: Ok(value!(23)),
            tdef: TypeDef::integer().fallible(),
        }

        invalid_facility_1 {
            args: func_args![value: value!("oopsie")],
            want: Err("syslog facility 'oopsie' not valid"),
            tdef: TypeDef::integer().fallible(),
        }

        invalid_facility_2 {
            args: func_args![value: value!("aww schucks")],
            want: Err("syslog facility 'aww schucks' not valid"),
            tdef: TypeDef::integer().fallible(),
        }
    ];
}
