use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use std::collections::BTreeMap;
    use std::net::SocketAddr;
    use std::str::FromStr;

    use hickory_client::client::{Client, SyncClient};
    use hickory_client::op::{DnsResponse, MessageType};
    use hickory_client::rr::{Name, Record};
    use hickory_client::udp::UdpClientConnection;

    use crate::compiler::prelude::*;
    use crate::value::Value;

    fn dns_lookup(
        value: Value,
        qtype: Value,
        qclass: Value,
        nameserver: Value,
        options: Value,
    ) -> Resolved {
        let nameserver: SocketAddr = nameserver
            .try_bytes_utf8_lossy()?
            .parse()
            .map_err(|err| format!("parsing nameserver failed: {err}"))?;
        let conn = UdpClientConnection::new(nameserver)
            .map_err(|err| format!("connecting to nameserver failed: {err}"))?;
        let client = SyncClient::new(conn);

        let host = Name::from_str(&value.try_bytes_utf8_lossy()?)
            .map_err(|err| format!("parsing host name failed: {err}"))?;
        let qtype = qtype
            .try_bytes_utf8_lossy()?
            .to_string()
            .parse()
            .map_err(|err| format!("parsing query type failed: {err}"))?;
        let qclass = qclass
            .try_bytes_utf8_lossy()?
            .to_string()
            .parse()
            .map_err(|err| format!("parsing query class failed: {err}"))?;

        let response = client
            .query(&host, qclass, qtype)
            .map_err(|err| format!("query failed: {err}"))?;

        println!("{}", options);

        Ok(parse_response(response)?.into())
    }

    #[derive(Debug, Clone)]
    pub(super) struct DnsLookupFn {
        pub(super) value: Box<dyn Expression>,
        pub(super) nameserver: Box<dyn Expression>,
        pub(super) qtype: Box<dyn Expression>,
        pub(super) class: Box<dyn Expression>,
        pub(super) options: Box<dyn Expression>,
    }

    impl Default for DnsLookupFn {
        fn default() -> Self {
            Self {
                value: expr!(""),
                nameserver: expr!(""),
                qtype: expr!("A"),
                class: expr!("IN"),
                options: expr!({}),
            }
        }
    }

    fn parse_response(answer: DnsResponse) -> Result<ObjectMap, ExpressionError> {
        let mut result = ObjectMap::new();
        let header_section = answer.header();
        let rcode = header_section.response_code();
        result.insert("fullRcode".into(), u16::from(rcode).into());
        result.insert("rcodeName".into(), rcode.to_string().into());
        let header = {
            let mut header_obj = ObjectMap::new();
            header_obj.insert("aa".into(), header_section.authoritative().into());
            header_obj.insert("ad".into(), header_section.authentic_data().into());
            header_obj.insert("cd".into(), header_section.checking_disabled().into());
            header_obj.insert("ra".into(), header_section.recursion_available().into());
            header_obj.insert("rd".into(), header_section.recursion_desired().into());
            header_obj.insert("tc".into(), header_section.truncated().into());
            header_obj.insert(
                "qr".into(),
                matches!(header_section.message_type(), MessageType::Query).into(),
            );
            header_obj.insert("id".into(), header_section.id().into());
            header_obj.insert("opcode".into(), u8::from(header_section.op_code()).into());
            header_obj.insert("rcode".into(), header_section.response_code().low().into());
            header_obj.insert("anCount".into(), answer.answer_count().into());
            header_obj.insert("arCount".into(), answer.additional_count().into());
            header_obj.insert("nsCount".into(), answer.name_server_count().into());
            header_obj.insert("qdCount".into(), answer.query_count().into());
            header_obj
        };
        result.insert("header".into(), header.into());

        let question = answer.queries();
        let answer_section = answer.answers();
        let authority = answer.name_servers();
        let additional = answer.additionals();

        let question = {
            let mut questions = Vec::<ObjectMap>::new();
            for q in question {
                let mut question_obj = ObjectMap::new();
                question_obj.insert("class".into(), q.query_class().to_string().into());
                question_obj.insert("domainName".into(), q.name().to_string().into());
                let qtype = q.query_type();
                question_obj.insert("questionType".into(), qtype.to_string().into());
                question_obj.insert("questionTypeId".into(), u16::from(qtype).into());
                questions.push(question_obj);
            }
            questions
        };
        result.insert("question".into(), question.into());
        result.insert(
            "answers".into(),
            parse_record_section(answer_section)?.into(),
        );
        result.insert("authority".into(), parse_record_section(authority)?.into());
        result.insert(
            "additional".into(),
            parse_record_section(additional)?.into(),
        );

        Ok(result)
    }

    fn parse_record_section(section: &[Record]) -> Result<Vec<ObjectMap>, ExpressionError> {
        let mut records = Vec::<ObjectMap>::new();
        for r in section {
            let mut record_obj = ObjectMap::new();
            record_obj.insert("class".into(), r.dns_class().to_string().into());
            record_obj.insert("domainName".into(), r.name().to_string().into());
            let rtype = r.record_type();
            let record_data = r.data().map(|r| r.to_string());
            record_obj.insert("rData".into(), record_data.into());
            record_obj.insert("recordType".into(), rtype.to_string().into());
            record_obj.insert("recordTypeId".into(), u16::from(rtype).into());
            record_obj.insert("ttl".into(), r.ttl().into());
            records.push(record_obj);
        }
        Ok(records)
    }

    impl FunctionExpression for DnsLookupFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let qtype = self.qtype.resolve(ctx)?;
            let class = self.class.resolve(ctx)?;
            let nameserver = self.nameserver.resolve(ctx)?;
            let options = self.options.resolve(ctx)?;
            dns_lookup(value, qtype, class, nameserver, options)
        }

        fn type_def(&self, _: &state::TypeState) -> TypeDef {
            TypeDef::object(inner_kind()).fallible()
        }
    }

    fn header_kind() -> BTreeMap<Field, Kind> {
        BTreeMap::from([
            (Field::from("aa"), Kind::boolean()),
            (Field::from("ad"), Kind::boolean()),
            (Field::from("anCount"), Kind::integer()),
            (Field::from("arCount"), Kind::integer()),
            (Field::from("cd"), Kind::boolean()),
            (Field::from("id"), Kind::integer()),
            (Field::from("nsCount"), Kind::integer()),
            (Field::from("opcode"), Kind::integer()),
            (Field::from("qdCount"), Kind::integer()),
            (Field::from("qr"), Kind::integer()),
            (Field::from("ra"), Kind::boolean()),
            (Field::from("rcode"), Kind::integer()),
            (Field::from("rd"), Kind::boolean()),
            (Field::from("tc"), Kind::boolean()),
        ])
    }

    fn rdata_kind() -> BTreeMap<Field, Kind> {
        BTreeMap::from([
            (Field::from("class"), Kind::bytes()),
            (Field::from("domainName"), Kind::bytes()),
            (Field::from("rData"), Kind::bytes()),
            (Field::from("recordType"), Kind::bytes()),
            (Field::from("recordTypeId"), Kind::integer()),
            (Field::from("ttl"), Kind::integer()),
        ])
    }

    fn question_kind() -> BTreeMap<Field, Kind> {
        BTreeMap::from([
            (Field::from("class"), Kind::bytes()),
            (Field::from("domainName"), Kind::bytes()),
            (Field::from("questionType"), Kind::bytes()),
            (Field::from("questionTypeId"), Kind::integer()),
        ])
    }

    pub(super) fn inner_kind() -> BTreeMap<Field, Kind> {
        BTreeMap::from([
            (Field::from("fullRcode"), Kind::integer()),
            (Field::from("rcodeName"), Kind::bytes() | Kind::null()),
            (Field::from("time"), Kind::bytes() | Kind::null()),
            (Field::from("timePrecision"), Kind::bytes() | Kind::null()),
            (
                Field::from("answers"),
                Kind::array(Collection::from_unknown(Kind::object(rdata_kind()))),
            ),
            (
                Field::from("authority"),
                Kind::array(Collection::from_unknown(Kind::object(rdata_kind()))),
            ),
            (
                Field::from("additional"),
                Kind::array(Collection::from_unknown(Kind::object(rdata_kind()))),
            ),
            (Field::from("header"), Kind::object(header_kind())),
            (
                Field::from("question"),
                Kind::array(Collection::from_unknown(Kind::object(question_kind()))),
            ),
        ])
    }
}

#[allow(clippy::wildcard_imports)]
#[cfg(not(target_arch = "wasm32"))]
use non_wasm::*;

#[derive(Clone, Copy, Debug)]
pub struct DnsLookup;

impl Function for DnsLookup {
    fn identifier(&self) -> &'static str {
        "dns_lookup"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "nameserver",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "qtype",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "class",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "options",
                kind: kind::OBJECT,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        // TODO: add
        &[Example {
            title: "Example",
            source: r#"dns_lookup!("localhost", nameserver: "127.0.0.53:53")"#,
            result: Ok(r#"["127.0.0.1"]"#),
        }]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let nameserver = arguments.required("nameserver");
        let qtype = arguments.optional("qtype").unwrap_or_else(|| expr!("A"));
        let class = arguments.optional("class").unwrap_or_else(|| expr!("IN"));
        let options = arguments.optional("options").unwrap_or_else(|| expr!({}));

        Ok(DnsLookupFn {
            value,
            nameserver,
            qtype,
            class,
            options,
        }
        .as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(ctx.span(), TypeDef::bytes().fallible()).as_expr())
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use std::collections::{BTreeMap, HashSet};

    use super::*;
    use crate::value;

    #[test]
    fn test_invalid_name() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("wrong.local"),
            nameserver: expr!("127.0.0.53:53"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(2));
        assert_eq!(result["rcodeName"], value!("Server Failure"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "wrong.local."
            })
        );
    }

    #[test]
    fn test_localhost() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("localhost"),
            nameserver: expr!("127.0.0.53:53"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(0));
        assert_eq!(result["rcodeName"], value!("No Error"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "localhost."
            })
        );
        let answer = result["answers"].as_array_unwrap()[0].as_object().unwrap();
        assert_eq!(answer["rData"], value!("127.0.0.1"));
    }

    #[test]
    fn test_google() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("dns.google"),
            nameserver: expr!("127.0.0.53:53"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(0));
        assert_eq!(result["rcodeName"], value!("No Error"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "dns.google."
            })
        );
        let answers: HashSet<String> = result["answers"]
            .as_array_unwrap()
            .iter()
            .map(|answer| {
                answer.as_object().unwrap()["rData"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        let expected: HashSet<String> = vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()]
            .into_iter()
            .collect();
        assert_eq!(answers, expected);
    }

    fn execute_dns_lookup(dns_lookup_fn: DnsLookupFn) -> ObjectMap {
        let tz = TimeZone::default();
        let mut object: Value = Value::Object(BTreeMap::new());
        let mut runtime_state = state::RuntimeState::default();
        let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
        dns_lookup_fn
            .resolve(&mut ctx)
            .map_err(|e| format!("{:#}", anyhow::anyhow!(e)))
            .unwrap()
            .try_object()
            .unwrap()
    }
}
