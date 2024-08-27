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

    let timestamp_millis = parsed.id.timestamp as i64;
    // Should we use UTC as the timezone? The logs are generated with the system's
    // timezone... What is the correct behavior? Maybe the system where vector is running
    // have a different timezone than the system that generated the logs... so it is
    // not correct to assume that the current system's timezone is the correct one
    // (with TimeZone::timestamp_millis_opt)
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
            // TODO: remove this clone with a new auditd crate version
            let value = Value::from(value.clone());
            (key, value)
        })
        // partition whether the key is all caps or not
        .partition(|(key, _)| key.chars().all(char::is_uppercase));

    log.insert("body".into(), Value::from(body));
    log.insert("enrichment".into(), Value::from(enrichment));

    Ok(Value::from(log))
}

impl From<AuditValue<'_>> for Value {
    fn from(value: AuditValue) -> Self {
        match value {
            AuditValue::Str(buf, _) => Value::from(buf),
            AuditValue::Number(num) => Value::from(num),
            value => Value::from(format!("TODO {:?}", value)),
        }
    }
}

impl From<Number> for Value {
    fn from(number: Number) -> Self {
        match number {
            Number::Dec(decimal) => Value::from(decimal),
            // TODO: should we store hexadecimals as its integer value or as an hexadecimal string?
            // Uppercase hexa or lowercase hex format?
            Number::Hex(hex) => Value::from(format!("0x{hex:X}")),
            // TODO: should we store octals as its integer value or as an octal string?
            Number::Oct(oct) => Value::from(format!("0o{oct:o}")),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
enum Error {
    #[error("timestamp (in milliseconds) {0} is out of range")]
    TimestampOutOfRange(i64),
}

impl From<Error> for ExpressionError {
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
    }
}
