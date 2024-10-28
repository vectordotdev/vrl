//! # DNS Lookup Function
//!
//! This function provides DNS lookup capabilities but is not recommended for frequent or performance-critical workflows.
//! It performs network calls, relying on a single-threaded worker that blocks on each request
//! until a response is received, which can degrade performance in high-throughput applications.
//!
//! Due to the potential for network-related delays or failures, avoid using this function
//! in latency-sensitive contexts.

use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use std::collections::BTreeMap;
    use std::io::Error;
    use std::net::ToSocketAddrs;
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use domain::base::iana::Class;
    use domain::base::{Name, RecordSection, Rtype};
    use domain::rdata::AllRecordData;
    use domain::resolv::stub::conf::{ResolvConf, ResolvOptions, ServerConf, Transport};
    use domain::resolv::stub::Answer;
    use domain::resolv::StubResolver;
    use once_cell::sync::Lazy;
    use tokio::runtime::Handle;

    use crate::compiler::prelude::*;
    use crate::value::Value;

    /// Single threaded worker for executing DNS requests.
    /// Currently blocks on each request until result is received.
    static WORKER: Lazy<Worker> = Lazy::new(Worker::new);
    const CHANNEL_CAPACITY: usize = 100;

    type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

    struct Worker {
        thread: Option<thread::JoinHandle<()>>,
        queue: Option<mpsc::SyncSender<Job<Result<Answer, Error>>>>,
        result_receiver: Option<Mutex<mpsc::Receiver<Result<Answer, Error>>>>,
    }

    impl Worker {
        // Creates a thread and 2 channels - one for jobs and one for results
        fn new() -> Self {
            let (sender, receiver) =
                mpsc::sync_channel::<Job<Result<Answer, Error>>>(CHANNEL_CAPACITY);
            let (result_sender, result_receiver) =
                mpsc::sync_channel::<Result<Answer, Error>>(CHANNEL_CAPACITY);
            let receiver = Arc::new(Mutex::new(receiver));
            Self {
                thread: Some(thread::spawn(move || loop {
                    let job = receiver
                        .lock()
                        .expect("Locking job queue failed")
                        .recv()
                        .expect("Worker queue closed");
                    let result = job();
                    result_sender
                        .send(result)
                        .expect("Sending result back from worker failed");
                })),
                queue: Some(sender),
                result_receiver: Some(result_receiver.into()),
            }
        }

        // Sends a job to the worker
        // Blocks until result is received
        fn execute<F>(&self, f: F) -> Result<Answer, Error>
        where
            F: FnOnce() -> Result<Answer, Error> + Send + 'static,
        {
            let job = Box::new(f);

            self.queue
                .as_ref()
                .expect("Expected queue to be present in the worker")
                .send(job)
                .expect("Submitting job to the queue failed");
            return self
                .result_receiver
                .as_ref()
                .expect("Expected result queue to be present in the worker")
                .lock()
                .expect("Locking result receiver failed")
                .recv()
                .expect("Job result channel closed");
        }
    }

    // Custom drop implementation which stops the started thread
    impl Drop for Worker {
        fn drop(&mut self) {
            drop(self.queue.take());
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
            drop(self.result_receiver.take());
        }
    }

    fn dns_lookup(value: Value, qtype: Value, qclass: Value, options: Value) -> Resolved {
        let host: Name<Vec<_>> = value
            .try_bytes_utf8_lossy()?
            .to_string()
            .parse()
            .map_err(|err| format!("parsing host name failed: {err}"))?;
        let qtype: Rtype = qtype
            .try_bytes_utf8_lossy()?
            .to_string()
            .parse()
            .map_err(|err| format!("parsing query type failed: {err}"))?;
        let qclass: Class = qclass
            .try_bytes_utf8_lossy()?
            .to_string()
            .parse()
            .map_err(|err| format!("parsing query class failed: {err}"))?;

        let conf = build_options(options.try_object()?)?;
        let answer = match Handle::try_current() {
            Ok(_) => WORKER.execute(move || {
                StubResolver::run_with_conf(conf, move |stub| async move {
                    stub.query((host, qtype, qclass)).await
                })
            }),
            Err(_) => StubResolver::run_with_conf(conf, move |stub| async move {
                stub.query((host, qtype, qclass)).await
            }),
        }
        .map_err(|err| format!("query failed: {err}"))?;

        Ok(parse_answer(answer)?.into())
    }

    #[derive(Debug, Clone)]
    pub(super) struct DnsLookupFn {
        pub(super) value: Box<dyn Expression>,
        pub(super) qtype: Box<dyn Expression>,
        pub(super) class: Box<dyn Expression>,
        pub(super) options: Box<dyn Expression>,
    }

    impl Default for DnsLookupFn {
        fn default() -> Self {
            Self {
                value: expr!(""),
                qtype: expr!("A"),
                class: expr!("IN"),
                options: expr!({}),
            }
        }
    }

    fn build_options(options: ObjectMap) -> Result<ResolvConf, ExpressionError> {
        let mut resolv_options = ResolvOptions::default();

        macro_rules! read_bool_opt {
            ($name:ident, $resolv_name:ident) => {
                if let Some($name) = options
                    .get(stringify!($name))
                    .map(|v| v.clone().try_boolean())
                    .transpose()?
                {
                    resolv_options.$resolv_name = $name;
                }
            };
            ($name:ident) => {
                read_bool_opt!($name, $name);
            };
        }

        macro_rules! read_int_opt {
            ($name:ident, $resolv_name:ident) => {
                if let Some($name) = options
                    .get(stringify!($name))
                    .map(|v| v.clone().try_integer())
                    .transpose()?
                {
                    resolv_options.$resolv_name = $name.try_into().map_err(|err| {
                        format!(
                            "{} has to be a positive integer, got: {}. ({})",
                            stringify!($resolv_name),
                            $name,
                            err
                        )
                    })?;
                }
            };
            ($name:ident) => {
                read_int_opt!($name, $name);
            };
        }

        read_int_opt!(ndots);
        read_int_opt!(attempts);
        read_bool_opt!(aa_only);
        read_bool_opt!(tcp, use_vc);
        read_bool_opt!(recurse);
        read_bool_opt!(rotate);

        if let Some(timeout) = options
            .get("timeout")
            .map(|v| v.clone().try_integer())
            .transpose()?
        {
            resolv_options.timeout = Duration::from_secs(timeout.try_into().map_err(|err| {
                format!("timeout has to be a positive integer, got: {timeout}. ({err})")
            })?);
        }

        let mut conf = ResolvConf {
            options: resolv_options,
            ..Default::default()
        };

        if let Some(servers) = options
            .get("servers")
            .map(|s| s.clone().try_array())
            .transpose()?
        {
            conf.servers.clear();
            for server in servers {
                let mut server = server.try_bytes_utf8_lossy()?;
                if !server.contains(':') {
                    server += ":53";
                }
                for addr in server
                    .to_socket_addrs()
                    .map_err(|err| format!("can't resolve nameserver ({server}): {err}"))?
                {
                    conf.servers.push(ServerConf::new(addr, Transport::UdpTcp));
                    conf.servers.push(ServerConf::new(addr, Transport::Tcp));
                }
            }
        }

        conf.finalize();
        Ok(conf)
    }

    fn parse_answer(answer: Answer) -> Result<ObjectMap, ExpressionError> {
        let mut result = ObjectMap::new();
        let header_section = answer.header();
        let rcode = header_section.rcode();
        result.insert("fullRcode".into(), rcode.to_int().into());
        result.insert("rcodeName".into(), rcode.to_string().into());
        let header = {
            let mut header_obj = ObjectMap::new();
            let counts = answer.header_counts();
            header_obj.insert("aa".into(), header_section.aa().into());
            header_obj.insert("ad".into(), header_section.ad().into());
            header_obj.insert("cd".into(), header_section.cd().into());
            header_obj.insert("ra".into(), header_section.ra().into());
            header_obj.insert("rd".into(), header_section.rd().into());
            header_obj.insert("tc".into(), header_section.tc().into());
            header_obj.insert("qr".into(), header_section.qr().into());
            header_obj.insert("opcode".into(), header_section.opcode().to_int().into());
            header_obj.insert("rcode".into(), header_section.rcode().to_int().into());
            header_obj.insert("anCount".into(), counts.ancount().into());
            header_obj.insert("arCount".into(), counts.arcount().into());
            header_obj.insert("nsCount".into(), counts.nscount().into());
            header_obj.insert("qdCount".into(), counts.qdcount().into());
            header_obj
        };
        result.insert("header".into(), header.into());

        let (question, answer_section, authority, additional) = answer
            .sections()
            .map_err(|err| format!("parsing response sections failed: {err}"))?;

        let question = {
            let mut questions = Vec::<ObjectMap>::new();
            for q in question {
                let q = q.map_err(|err| format!("parsing question section failed: {err}"))?;
                let mut question_obj = ObjectMap::new();
                question_obj.insert("class".into(), q.qclass().to_string().into());
                question_obj.insert("domainName".into(), q.qname().to_string().into());
                let qtype = q.qtype();
                question_obj.insert("questionType".into(), qtype.to_string().into());
                question_obj.insert("questionTypeId".into(), qtype.to_int().into());
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

    fn parse_record_section(
        section: RecordSection<'_, Bytes>,
    ) -> Result<Vec<ObjectMap>, ExpressionError> {
        let mut records = Vec::<ObjectMap>::new();
        for r in section {
            let r = r.map_err(|err| format!("parsing record section failed: {err}"))?;
            let mut record_obj = ObjectMap::new();
            record_obj.insert("class".into(), r.class().to_string().into());
            record_obj.insert("domainName".into(), r.owner().to_string().into());
            let rtype = r.rtype();
            let record_data = r
                .to_record::<AllRecordData<_, _>>()
                .map_err(|err| format!("parsing rData failed: {err}"))?
                .map(|r| r.data().to_string());
            record_obj.insert("rData".into(), record_data.into());
            record_obj.insert("recordType".into(), rtype.to_string().into());
            record_obj.insert("recordTypeId".into(), rtype.to_int().into());
            record_obj.insert("ttl".into(), r.ttl().as_secs().into());
            records.push(record_obj);
        }
        Ok(records)
    }

    impl FunctionExpression for DnsLookupFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let qtype = self.qtype.resolve(ctx)?;
            let class = self.class.resolve(ctx)?;
            let options = self.options.resolve(ctx)?;
            dns_lookup(value, qtype, class, options)
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

    #[cfg(not(feature = "test"))]
    fn examples(&self) -> &'static [Example] {
        &[]
    }

    #[cfg(feature = "test")]
    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Basic lookup",
                source: r#"
                    res = dns_lookup!("dns.google")
                    # reset non-static ttl so result is static
                    res.answers = map_values(res.answers) -> |value| {
                      value.ttl = 600
                      value
                    }
                    # remove extra responses for example
                    res.answers = filter(res.answers) -> |_, value| {
                        value.rData == "8.8.8.8"
                    }
                    # remove class since this is also dynamic
                    res.additional = map_values(res.additional) -> |value| {
                        del(value.class)
                        value
                    }
                    res
                    "#,
                result: Ok(indoc!(
                    r#"{
                    "additional": [
                      {
                        "domainName": "",
                        "rData": "OPT ...",
                        "recordType": "OPT",
                        "recordTypeId": 41,
                        "ttl": 0
                      }
                    ],
                    "answers": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "rData": "8.8.8.8",
                        "recordType": "A",
                        "recordTypeId": 1,
                        "ttl": 600
                      }
                    ],
                    "authority": [],
                    "fullRcode": 0,
                    "header": {
                      "aa": false,
                      "ad": false,
                      "anCount": 2,
                      "arCount": 1,
                      "cd": false,
                      "nsCount": 0,
                      "opcode": 0,
                      "qdCount": 1,
                      "qr": true,
                      "ra": true,
                      "rcode": 0,
                      "rd": true,
                      "tc": false
                    },
                    "question": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "questionType": "A",
                        "questionTypeId": 1
                      }
                    ],
                    "rcodeName": "NOERROR"
                  }"#
                )),
            },
            Example {
                title: "Custom class and qtype",
                source: r#"
                    res = dns_lookup!("dns.google", class: "IN", qtype: "A")
                    # reset non-static ttl so result is static
                    res.answers = map_values(res.answers) -> |value| {
                      value.ttl = 600
                      value
                    }
                    # remove extra responses for example
                    res.answers = filter(res.answers) -> |_, value| {
                        value.rData == "8.8.8.8"
                    }
                    # remove class since this is also dynamic
                    res.additional = map_values(res.additional) -> |value| {
                        del(value.class)
                        value
                    }
                    res
                    "#,
                result: Ok(indoc!(
                    r#"{
                    "additional": [
                      {
                        "domainName": "",
                        "rData": "OPT ...",
                        "recordType": "OPT",
                        "recordTypeId": 41,
                        "ttl": 0
                      }
                    ],
                    "answers": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "rData": "8.8.8.8",
                        "recordType": "A",
                        "recordTypeId": 1,
                        "ttl": 600
                      }
                    ],
                    "authority": [],
                    "fullRcode": 0,
                    "header": {
                      "aa": false,
                      "ad": false,
                      "anCount": 2,
                      "arCount": 1,
                      "cd": false,
                      "nsCount": 0,
                      "opcode": 0,
                      "qdCount": 1,
                      "qr": true,
                      "ra": true,
                      "rcode": 0,
                      "rd": true,
                      "tc": false
                    },
                    "question": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "questionType": "A",
                        "questionTypeId": 1
                      }
                    ],
                    "rcodeName": "NOERROR"
                  }"#
                )),
            },
            Example {
                title: "Custom options",
                source: r#"
                    res = dns_lookup!("dns.google", options: {"timeout": 30, "attempts": 5})
                    res.answers = map_values(res.answers) -> |value| {
                      value.ttl = 600
                      value
                    }
                    # remove extra responses for example
                    res.answers = filter(res.answers) -> |_, value| {
                        value.rData == "8.8.8.8"
                    }
                    # remove class since this is also dynamic
                    res.additional = map_values(res.additional) -> |value| {
                        del(value.class)
                        value
                    }
                    res
                    "#,
                result: Ok(indoc!(
                    r#"{
                    "additional": [
                      {
                        "domainName": "",
                        "rData": "OPT ...",
                        "recordType": "OPT",
                        "recordTypeId": 41,
                        "ttl": 0
                      }
                    ],
                    "answers": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "rData": "8.8.8.8",
                        "recordType": "A",
                        "recordTypeId": 1,
                        "ttl": 600
                      }
                    ],
                    "authority": [],
                    "fullRcode": 0,
                    "header": {
                      "aa": false,
                      "ad": false,
                      "anCount": 2,
                      "arCount": 1,
                      "cd": false,
                      "nsCount": 0,
                      "opcode": 0,
                      "qdCount": 1,
                      "qr": true,
                      "ra": true,
                      "rcode": 0,
                      "rd": true,
                      "tc": false
                    },
                    "question": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "questionType": "A",
                        "questionTypeId": 1
                      }
                    ],
                    "rcodeName": "NOERROR"
                  }"#
                )),
            },
            Example {
                title: "Custom server",
                source: r#"
                    res = dns_lookup!("dns.google", options: {"servers": ["dns.quad9.net"]})
                    res.answers = map_values(res.answers) -> |value| {
                      value.ttl = 600
                      value
                    }
                    # remove extra responses for example
                    res.answers = filter(res.answers) -> |_, value| {
                        value.rData == "8.8.8.8"
                    }
                    # remove class since this is also dynamic
                    res.additional = map_values(res.additional) -> |value| {
                        del(value.class)
                        value
                    }
                    res
                    "#,
                result: Ok(indoc!(
                    r#"{
                    "additional": [
                      {
                        "domainName": "",
                        "rData": "OPT ...",
                        "recordType": "OPT",
                        "recordTypeId": 41,
                        "ttl": 0
                      }
                    ],
                    "answers": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "rData": "8.8.8.8",
                        "recordType": "A",
                        "recordTypeId": 1,
                        "ttl": 600
                      }
                    ],
                    "authority": [],
                    "fullRcode": 0,
                    "header": {
                      "aa": false,
                      "ad": false,
                      "anCount": 2,
                      "arCount": 1,
                      "cd": false,
                      "nsCount": 0,
                      "opcode": 0,
                      "qdCount": 1,
                      "qr": true,
                      "ra": true,
                      "rcode": 0,
                      "rd": true,
                      "tc": false
                    },
                    "question": [
                      {
                        "class": "IN",
                        "domainName": "dns.google",
                        "questionType": "A",
                        "questionTypeId": 1
                      }
                    ],
                    "rcodeName": "NOERROR"
                  }"#
                )),
            },
        ]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let qtype = arguments.optional("qtype").unwrap_or_else(|| expr!("A"));
        let class = arguments.optional("class").unwrap_or_else(|| expr!("IN"));
        let options = arguments.optional("options").unwrap_or_else(|| expr!({}));

        Ok(DnsLookupFn {
            value,
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
            ..Default::default()
        });

        assert_ne!(result["fullRcode"], value!(0));
        assert_ne!(result["rcodeName"], value!("NOERROR"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "wrong.local"
            })
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    // MacOS resolver doesn't always handle localhost
    fn test_localhost() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("localhost"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(0));
        assert_eq!(result["rcodeName"], value!("NOERROR"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "localhost"
            })
        );
        let answer = result["answers"].as_array_unwrap()[0].as_object().unwrap();
        assert_eq!(answer["rData"], value!("127.0.0.1"));
    }

    #[test]
    fn test_custom_type() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("google.com"),
            qtype: expr!("mx"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(0));
        assert_eq!(result["rcodeName"], value!("NOERROR"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 15,
                "questionType": "MX",
                "class": "IN",
                "domainName": "google.com"
            })
        );
    }

    #[test]
    fn test_google() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("dns.google"),
            ..Default::default()
        });

        assert_eq!(result["fullRcode"], value!(0));
        assert_eq!(result["rcodeName"], value!("NOERROR"));
        assert_eq!(
            result["question"].as_array_unwrap()[0],
            value!({
                "questionTypeId": 1,
                "questionType": "A",
                "class": "IN",
                "domainName": "dns.google"
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

    #[test]
    fn unknown_options_ignored() {
        let result = execute_dns_lookup(DnsLookupFn {
            value: expr!("dns.google"),
            options: expr!({"test": "test"}),
            ..Default::default()
        });

        assert_eq!(result["rcodeName"], value!("NOERROR"));
    }

    #[test]
    fn invalid_option_type() {
        let result = execute_dns_lookup_with_expected_error(DnsLookupFn {
            value: expr!("dns.google"),
            options: expr!({"tcp": "yes"}),
            ..Default::default()
        });

        assert_eq!(result.message(), "expected boolean, got string");
    }

    #[test]
    fn negative_int_type() {
        let attempts_val = -5;
        let result = execute_dns_lookup_with_expected_error(DnsLookupFn {
            value: expr!("dns.google"),
            options: expr!({"attempts": attempts_val}),
            ..Default::default()
        });

        assert_eq!(
            result.message(),
            "attempts has to be a positive integer, got: -5. (out of range integral type conversion attempted)"
        );
    }

    fn prepare_dns_lookup(dns_lookup_fn: DnsLookupFn) -> Resolved {
        let tz = TimeZone::default();
        let mut object: Value = Value::Object(BTreeMap::new());
        let mut runtime_state = state::RuntimeState::default();
        let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
        dns_lookup_fn.resolve(&mut ctx)
    }

    fn execute_dns_lookup(dns_lookup_fn: DnsLookupFn) -> ObjectMap {
        prepare_dns_lookup(dns_lookup_fn)
            .map_err(|e| format!("{:#}", anyhow::anyhow!(e)))
            .unwrap()
            .try_object()
            .unwrap()
    }

    fn execute_dns_lookup_with_expected_error(dns_lookup_fn: DnsLookupFn) -> ExpressionError {
        prepare_dns_lookup(dns_lookup_fn).unwrap_err()
    }
}
