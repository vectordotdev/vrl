use crate::compiler::prelude::*;
use prost_reflect::ReflectMessage;
use prost_reflect::{DynamicMessage, MessageDescriptor};

pub fn proto_to_value(
    prost_reflect_value: &prost_reflect::Value,
    field_descriptor: Option<&prost_reflect::FieldDescriptor>,
) -> std::result::Result<Value, String> {
    let vrl_value = match prost_reflect_value {
        prost_reflect::Value::Bool(v) => Value::from(*v),
        prost_reflect::Value::I32(v) => Value::from(*v),
        prost_reflect::Value::I64(v) => Value::from(*v),
        prost_reflect::Value::U32(v) => Value::from(*v),
        prost_reflect::Value::U64(v) => Value::from(*v),
        prost_reflect::Value::F32(v) => {
            Value::Float(NotNan::new(f64::from(*v)).map_err(|_e| "Float number cannot be Nan")?)
        }
        prost_reflect::Value::F64(v) => {
            Value::Float(NotNan::new(*v).map_err(|_e| "F64 number cannot be Nan")?)
        }
        prost_reflect::Value::String(v) => Value::from(v.as_str()),
        prost_reflect::Value::Bytes(v) => Value::from(v.clone()),
        prost_reflect::Value::EnumNumber(v) => {
            if let Some(field_descriptor) = field_descriptor {
                let kind = field_descriptor.kind();
                let enum_desc = kind.as_enum().ok_or_else(|| {
                    format!(
                        "Internal error while parsing protobuf enum. Field descriptor: {:?}",
                        field_descriptor
                    )
                })?;
                Value::from(
                    enum_desc
                        .get_value(*v)
                        .ok_or_else(|| {
                            format!("The number {} cannot be in '{}'", v, enum_desc.name())
                        })?
                        .name(),
                )
            } else {
                Err("Expected valid field descriptor")?
            }
        }
        prost_reflect::Value::Message(v) => {
            let mut obj_map = ObjectMap::new();
            for field_desc in v.descriptor().fields() {
                if v.has_field(&field_desc) {
                    let field_value = v.get_field(&field_desc);
                    let out = proto_to_value(field_value.as_ref(), Some(&field_desc))?;
                    obj_map.insert(field_desc.name().into(), out);
                }
            }
            Value::from(obj_map)
        }
        prost_reflect::Value::List(v) => {
            let vec = v
                .iter()
                .map(|o| proto_to_value(o, field_descriptor))
                .collect::<Result<Vec<_>, String>>()?;
            Value::from(vec)
        }
        prost_reflect::Value::Map(v) => {
            if let Some(field_descriptor) = field_descriptor {
                let kind = field_descriptor.kind();
                let message_desc = kind.as_message().ok_or_else(|| {
                    format!(
                        "Internal error while parsing protobuf field descriptor: {:?}",
                        field_descriptor
                    )
                })?;
                Value::from(
                    v.iter()
                        .map(|kv| {
                            Ok((
                                kv.0.as_str()
                                    .ok_or_else(|| {
                                        format!(
                                            "Internal error while parsing protobuf map. Field descriptor: {:?}",
                                            field_descriptor
                                        )
                                    })?
                                    .into(),
                                proto_to_value(kv.1, Some(&message_desc.map_entry_value_field()))?,
                            ))
                        })
                        .collect::<std::result::Result<ObjectMap, String>>()?,
                )
            } else {
                Err("Expected valid field descriptor")?
            }
        }
    };
    Ok(vrl_value)
}

pub(crate) fn parse_proto(descriptor: &MessageDescriptor, value: Value) -> Resolved {
    let bytes = value.try_bytes()?;

    let dynamic_message = DynamicMessage::decode(descriptor.clone(), bytes)
        .map_err(|error| format!("Error parsing protobuf: {:?}", error))?;
    Ok(proto_to_value(
        &prost_reflect::Value::Message(dynamic_message),
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobuf::get_message_descriptor;
    use crate::value;
    use std::path::PathBuf;
    use std::{env, fs};

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("tests/data/protobuf")
    }

    fn read_pb_file(protobuf_bin_message_path: &str) -> String {
        fs::read_to_string(test_data_dir().join(protobuf_bin_message_path)).unwrap()
    }

    #[test]
    fn test_parse_files() {
        let path = test_data_dir().join("test_protobuf.desc");
        let descriptor = get_message_descriptor(&path, "test_protobuf.Person").unwrap();
        let encoded_value = value!(read_pb_file("person_someone.pb"));
        let parsed_value = parse_proto(&descriptor, encoded_value);
        assert!(
            parsed_value.is_ok(),
            "Failed to parse proto: {:?}",
            parsed_value.unwrap_err()
        );
        let parsed_value = parsed_value.unwrap();
        let value = value!({ name: "someone", phones: [{number: "123456"}] });
        assert_eq!(value, parsed_value)
    }

    #[test]
    fn test_parse_proto3() {
        let path = test_data_dir().join("test_protobuf3.desc");
        let descriptor = get_message_descriptor(&path, "test_protobuf3.Person").unwrap();
        let encoded_value = value!(read_pb_file("person_someone3.pb"));
        let parsed_value = parse_proto(&descriptor, encoded_value);
        assert!(
            parsed_value.is_ok(),
            "Failed to parse proto: {:?}",
            parsed_value.unwrap_err()
        );
        let parsed_value = parsed_value.unwrap();
        let value = value!({ data: {data_phone: "HOME"}, name: "someone", phones: [{number: "1234", type: "MOBILE"}] });
        assert_eq!(value, parsed_value)
    }
}
