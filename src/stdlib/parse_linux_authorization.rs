use super::parse_syslog::ParseSyslogFn;
use crate::compiler::prelude::*;
use chrono::{Datelike, Utc};
use std::sync::LazyLock;

static EXAMPLES: LazyLock<Vec<Example>> = LazyLock::new(|| {
    let result = Box::leak(
        format!(
            indoc! {r#"{{
                "appname": "sshd",
                "hostname": "localhost",
                "message": "Accepted publickey for eng from 10.1.1.1 port 8888 ssh2: RSA SHA256:foobar",
                "procid": 1111,
                "timestamp": "{year}-03-23T01:49:58Z"
            }}"#},
            year = Utc::now().year()
        )
        .into_boxed_str(),
    );
    vec![example! {
        title: "Parse Linux authorization event",
        source: indoc! {"
            parse_linux_authorization!(
                s'Mar 23 01:49:58 localhost sshd[1111]: Accepted publickey for eng from 10.1.1.1 port 8888 ssh2: RSA SHA256:foobar'
            )
        "},
        result: Ok(result),
    }]
});

#[derive(Clone, Copy, Debug)]
pub struct ParseLinuxAuthorization;

impl Function for ParseLinuxAuthorization {
    fn identifier(&self) -> &'static str {
        "parse_linux_authorization"
    }

    fn usage(&self) -> &'static str {
        "Parses Linux authorization logs usually found under either `/var/log/auth.log` (for Debian-based systems) or `/var/log/secure` (for RedHat-based systems) according to [Syslog](https://en.wikipedia.org/wiki/Syslog) format."
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a properly formatted Syslog message."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            The function resolves the year for messages that don't include it. If the current month
            is January, and the message is for December, it will take the previous year. Otherwise,
            take the current year.
        "}]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The text containing the message to parse.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        EXAMPLES.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        // The parse_linux_authorization function is just an alias for parse_syslog
        Ok(ParseSyslogFn { value }.as_expr())
    }
}
