use crate::compiler::prelude::*;
use bytes::BytesMut;
use chrono::DateTime;
use linux_audit_parser::{Number, Value as AuditValue};

fn parse_auditd(bytes: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    // check if bytes ends with newline, otherwise append it
    // TODO: make the parser accept bytes without newline in the linux_audit_parser crate
    let bytes = if bytes.last() == Some(&b'\n') {
        bytes
    } else {
        let mut bytes = BytesMut::from(bytes);
        bytes.extend_from_slice(b"\n");
        bytes.freeze()
    };
    let parsed = linux_audit_parser::parse(&bytes, false)?;

    let mut log = ObjectMap::new();

    let timestamp_millis = i64::try_from(parsed.id.timestamp)
        .map_err(|_| Error::TimestampOverflow(parsed.id.timestamp))?;
    // Should we use UTC as the timezone? The logs are generated with the system's
    // timezone... What is the correct behavior? Maybe the system where vector is running
    // have a different timezone than the system that generated the logs... so it is
    // not correct to assume that the current system's timezone is the correct one
    // (with TimeZone::timestamp_millis_opt)
    // TODO: we should document that the timestamp is parsed into UTC timezone.
    // TODO: Maybe we should accept in a parameter a custom timezone to parse the timestamp?
    let Some(timestamp) = DateTime::from_timestamp_millis(timestamp_millis) else {
        return Err(Error::TimestampOutOfRange(timestamp_millis).into());
    };
    log.insert("timestamp".into(), Value::from(timestamp));

    let sequence = parsed.id.sequence;
    log.insert("sequence".into(), Value::from(sequence));

    if let Some(node) = parsed.node {
        log.insert("node".into(), Value::from(node));
    }

    let message_type = parsed.ty.to_string();
    log.insert("type".into(), Value::from(message_type));

    let (enrichment, body): (ObjectMap, ObjectMap) = parsed
        .body
        .into_iter()
        // TODO: improve this auditd crate with a IntoIter implementation for Body and not only
        // for &Body, so we can have owned values
        .map(|(key, value)| {
            let key = KeyString::from(key.to_string());
            // TODO: remove this clone with a new auditd crate version (to-be-contributed yet)
            let value = Value::try_from(value.clone());
            value.map(|value| (key, value))
        })
        .collect::<Result<Vec<_>, _>>()?
        // partition whether the key is all caps or not
        .into_iter()
        .partition(|(key, _)| key.chars().all(char::is_uppercase));

    log.insert("body".into(), Value::from(body));
    // TODO: do not insert enrichment if it is empty. Create a test for this
    log.insert("enrichment".into(), Value::from(enrichment));

    Ok(Value::from(log))
}

impl<'a> TryFrom<AuditValue<'a>> for Value {
    type Error = Error<'a>;
    fn try_from(value: AuditValue<'a>) -> Result<Self, Self::Error> {
        let result = match value {
            AuditValue::Empty => Value::Null,
            AuditValue::Str(string, _) => Value::from(string),
            AuditValue::Number(num) => Value::from(num),
            AuditValue::List(values) => Value::from(
                values
                    .into_iter()
                    .map(Value::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            AuditValue::Owned(string) => Value::from(string),
            // Currently, AuditValue::Map is not generated from the parser (https://github.com/hillu/linux-audit-parser-rs/blob/d8c448c8d8227467b81cd5267790415b8b73f0cb/src/value.rs#L70)
            // but I think we should use that data type to represent the key-value pairs of the internal (nested) msg field
            // as depicted here: https://github.com/vectordotdev/vrl/issues/66
            // For example, in the following linux-audit-parser test https://github.com/hillu/linux-audit-parser-rs/blob/d8c448c8d8227467b81cd5267790415b8b73f0cb/src/test.rs#L168
            // we see that the msg field is parsed as a string, although it could be key-value pairs
            AuditValue::Map(entries) => Value::from(
                entries
                    .into_iter()
                    .map(|(key, value)| {
                        Value::try_from(value).map(|value| (key.to_string().into(), value))
                    })
                    .collect::<Result<ObjectMap, _>>()?,
            ),
            // There is a few values that `linux-audit-parser` does not return in its parsing
            // https://github.com/hillu/linux-audit-parser-rs/blob/d8c448c8d8227467b81cd5267790415b8b73f0cb/src/value.rs#L72
            // We do not plan to support those values, as they are only produced by [laurel](https://github.com/threathunters-io/laurel)
            // Maybe we should contribute to `linux-audit-parser` to remove the values the parser does not produce, as it
            // does not have sense in this context.
            // If those variants are removed, we can remove the error handling of this cases are simplify the code.
            // Otherwise, we could simply panic in this case
            unsupported_value => return Err(Error::UnsupportedValue(unsupported_value)),
        };
        Ok(result)
    }
}

impl From<Number> for Value {
    fn from(number: Number) -> Self {
        match number {
            Number::Dec(decimal) => Value::from(decimal),
            // TODO: should we store hexadecimals as its integer value or as an hexadecimal string?
            // TODO: Uppercase hexa or lowercase hex format?
            Number::Hex(hex) => Value::from(format!("0x{hex:X}")),
            // TODO: should we store octals as its integer value or as an octal string?
            Number::Oct(oct) => Value::from(format!("0o{oct:o}")),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error<'a> {
    #[error("timestamp (in milliseconds) {0} is out of range")]
    TimestampOutOfRange(i64),
    #[error("timestamp {0} overflow while converting from u64 to i64")]
    TimestampOverflow(u64),
    #[error("unsupported auditd value: {0:?}")]
    UnsupportedValue(AuditValue<'a>),
}

impl From<Error<'_>> for ExpressionError {
    fn from(error: Error) -> Self {
        Self::Error {
            message: format!("Error while converting parsed Auditd record to object: {error}"),
            labels: vec![],
            notes: vec![],
        }
    }
}

impl From<linux_audit_parser::ParseError> for ExpressionError {
    fn from(error: linux_audit_parser::ParseError) -> Self {
        Self::Error {
            message: format!("Auditd record parsing error: {error}"),
            labels: vec![],
            notes: vec![],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseAuditd;

impl Function for ParseAuditd {
    fn identifier(&self) -> &'static str {
        "parse_auditd"
    }

    fn summary(&self) -> &'static str {
        "Parse an auditd record"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        // TODO
        &[]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ParseAuditdFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct ParseAuditdFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseAuditdFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        parse_auditd(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        type_def()
    }
}

fn type_def() -> TypeDef {
    // TODO: improve typedef
    TypeDef::object(Collection::any())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auditd() {
        let line = r#"type=DAEMON_START msg=audit(1724423274.618:6439): op=start ver=4.0.2 format=enriched kernel=6.10.4-arch2-1 auid=1000 pid=1240242 uid=0 ses=2 res=successAUID="jorge" UID="root""#;
        let value = Value::from(line);
        println!("{}", parse_auditd(value).expect("expected ok "));

        let other_line = r#"type=SYSCALL msg=audit(1522927552.749:917): arch=c000003e syscall=2 success=yes exit=3 a0=7ffe2ce05793 a1=0 a2=1fffffffffff0000 a3=7ffe2ce043a0 items=1 ppid=2906 pid=4668 auid=1000 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=pts4 ses=1 comm="cat" exe="/bin/cat" key="passwd""#;
        let value = Value::from(other_line);
        println!("{}", parse_auditd(value).expect("expected ok "));
    }
}
