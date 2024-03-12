use crate::compiler::prelude::*;
use crate::protobuf::encode_proto;
use crate::protobuf::get_message_descriptor;
use prost_reflect::MessageDescriptor;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub struct EncodeProto;

impl Function for EncodeProto {
    fn identifier(&self) -> &'static str {
        "encode_proto"
    }

    fn summary(&self) -> &'static str {
        "Encodes a value into a protobuf"
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
                kind: kind::ANY,
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
                source: r#"encode_proto!({
                    "id": 42,
                    "name": "John Doe",
                    "address": {
                        "street": "123 Main St",
                        "city": "New York",
                        "country": "USA"
                    }
		}, "person.desc", "proto.Person")"#,
                result: Ok(
                    r#"base64_encode("CCoSCEpvaG4gRG9lGi4SDDEyMyBNYWluIFN0EghOZXcgWW9yaxoDVVNB")"#,
                ),
            },
            Example {
                title: "repeated fields",
                source: r#"encode_proto!({
                    "items": [
                        { "name": "item1", "quantity": 10 },
                        { "name": "item2", "quantity": 5 }
                    ]
                }, "order.desc", "proto.Order")"#,
                result: Ok(r#"base64_encode("EhIKBWl0ZW0xEAoSEQoFaXRlbTIQBQ==")"#),
            },
            Example {
                title: "enum field",
                source: r#"encode_proto!({
                    "status": "ACTIVE",
                    "name": "Project X"
                }, "project.desc", "proto.Project")"#,
                result: Ok(r#"base64_encode("CAESCVByb2plY3QgWA==")"#),
            },
            Example {
                title: "timestamp field",
                source: r#"encode_proto!({
                    "event_time": t'2023-05-26T10:30:00Z',
                    "message": "Event occurred"
                }, "event.desc", "proto.Event")"#,
                result: Ok(r#"base64_encode("CPDBrubNCRIOBUV2ZW50IG9jY3VycmVk")"#),
            },
            Example {
                title: "map field",
                source: r#"encode_proto!({
                    "labels": {
                        "key1": "value1",
                        "key2": "value2"
                    }
                }, "metadata.desc", "proto.Metadata")"#,
                result: Ok(r#"base64_encode("ChIKBGtleTESBnZhbHVlMQoEa2V5MhIGdmFsdWUy")"#),
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
            get_message_descriptor(&path, &message_type_str).expect("message type not found");

        Ok(EncodeProtoFn { descriptor, value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct EncodeProtoFn {
    descriptor: MessageDescriptor,
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeProtoFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        encode_proto(&self.descriptor, value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
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
        encode_proto => EncodeProto;

        encodes {
            args: func_args![ value: value!({ name: "someone", phones: [{number: "123456"}] }),
                desc_file: test_data_dir().join("test_protobuf.desc").to_str().unwrap().to_owned(),
                message_type: "test_protobuf.Person"],
            want: Ok(value!(read_pb_file("person_someone.pb"))),
            tdef: TypeDef::bytes().infallible(),
        }

        encodes_proto3 {
            args: func_args![ value: value!({ data: {data_phone: "HOME"}, name: "someone", phones: [{number: "1234", type: "MOBILE"}] }),
                desc_file: test_data_dir().join("test_protobuf3.desc").to_str().unwrap().to_owned(),
                message_type: "test_protobuf3.Person"],
            want: Ok(value!(read_pb_file("person_someone3.pb"))),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
