use crate::compiler::prelude::*;
use crate::protobuf::get_message_descriptor;
use crate::protobuf::parse_proto;
use prost_reflect::MessageDescriptor;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub struct ParseProto;

impl Function for ParseProto {
    fn identifier(&self) -> &'static str {
        "parse_proto"
    }

    fn summary(&self) -> &'static str {
        "parse a string to a protobuf based type"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Parses the provided `value` as protocol buffer.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "desc_file",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "message_type",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "message",
                source: r#"parse_proto!(base64_decode("CCoSCEpvaG4gRG9lGi4SDDEyMyBNYWluIFN0EghOZXcgWW9yaxoDVVNB"), "person.desc", "proto.Person")"#,
                result: Ok(
                    r#"{ "id": 42, "name": "John Doe", "address": { "street": "123 Main St", "city": "New York", "country": "USA" } }"#,
                ),
            },
            Example {
                title: "repeated fields",
                source: r#"parse_proto!(base64_decode("EhIKBWl0ZW0xEAoSEQoFaXRlbTIQBQ=="), "order.desc", "proto.Order")"#,
                result: Ok(
                    r#"{ "items": [ { "name": "item1", "quantity": 10 }, { "name": "item2", "quantity": 5 } ] }"#,
                ),
            },
            Example {
                title: "enum field",
                source: r#"parse_proto!(base64_decode("CAESCVByb2plY3QgWA=="), "project.desc", "proto.Project")"#,
                result: Ok(r#"{ "status": "ACTIVE", "name": "Project X" }"#),
            },
            Example {
                title: "timestamp field",
                source: r#"parse_proto!(base64_decode("CPDBrubNCRIOBUV2ZW50IG9jY3VycmVk"), "event.desc", "proto.Event")"#,
                result: Ok(
                    r#"{ "event_time": t'2023-05-26T10:30:00Z', "message": "Event occurred" }"#,
                ),
            },
            Example {
                title: "map field",
                source: r#"parse_proto!(base64_decode("ChIKBGtleTESBnZhbHVlMQoEa2V5MhIGdmFsdWUy"), "metadata.desc", "proto.Metadata")"#,
                result: Ok(r#"{ "labels": { "key1": "value1", "key2": "value2" } }"#),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let desc_file = arguments.required_literal("desc_file", state)?;
        let desc_file_str = desc_file
            .try_bytes_utf8_lossy()
            .expect("descriptor file must be a string");
        let message_type = arguments.required_literal("message_type", state)?;
        let message_type_str = message_type
            .try_bytes_utf8_lossy()
            .expect("message_type must be a string");
        let os_string: OsString = desc_file_str.into_owned().into();
        let path_buf = PathBuf::from(os_string);
        let path = Path::new(&path_buf);
        let descriptor =
            get_message_descriptor(path, &message_type_str).expect("message type not found");

        Ok(ParseProtoFn { descriptor, value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseProtoFn {
    descriptor: MessageDescriptor,
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseProtoFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_proto(&self.descriptor, value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        type_def()
    }
}

fn inner_kind() -> Kind {
    Kind::null()
        | Kind::bytes()
        | Kind::integer()
        | Kind::float()
        | Kind::boolean()
        | Kind::array(Collection::any())
        | Kind::object(Collection::any())
}

fn type_def() -> TypeDef {
    TypeDef::bytes()
        .fallible()
        .or_boolean()
        .or_integer()
        .or_float()
        .add_null()
        .or_array(Collection::from_unknown(inner_kind()))
        .or_object(Collection::from_unknown(inner_kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use std::{env, fs};

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("tests/data/protobuf")
    }

    fn read_pb_file(protobuf_bin_message_path: &str) -> String {
        fs::read_to_string(test_data_dir().join(protobuf_bin_message_path)).unwrap()
    }

    test_function![
        parse_proto => ParseProto;

        parses {
            args: func_args![ value: read_pb_file("person_someone.pb"),
                desc_file: test_data_dir().join("test_protobuf.desc").to_str().unwrap().to_owned(),
                message_type: "test_protobuf.Person"],
            want: Ok(value!({ name: "someone", phones: [{number: "123456"}] })),
            tdef: type_def(),
        }

        parses_proto3 {
            args: func_args![ value: read_pb_file("person_someone3.pb"),
                desc_file: test_data_dir().join("test_protobuf3.desc").to_str().unwrap().to_owned(),
                message_type: "test_protobuf3.Person"],
            want: Ok(value!({ data: {data_phone: "HOME"}, name: "someone", phones: [{number: "1234", type: "MOBILE"}] })),
            tdef: type_def(),
        }
    ];
}
