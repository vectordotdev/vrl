use crate::compiler::prelude::*;
use chrono::Timelike;
use prost::Message;
use prost_reflect::{DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor};
use std::collections::HashMap;

/// Convert a single raw `Value` into a protobuf `Value`.
///
/// Unlike `convert_value`, this ignores any field metadata such as cardinality.
fn convert_value_raw(
    value: Value,
    kind: &prost_reflect::Kind,
) -> std::result::Result<prost_reflect::Value, String> {
    let kind_str = value.kind_str().to_owned();
    match (value, kind) {
        (Value::Boolean(b), Kind::Bool) => Ok(prost_reflect::Value::Bool(b)),
        (Value::Bytes(b), Kind::Bytes) => Ok(prost_reflect::Value::Bytes(b)),
        (Value::Bytes(b), Kind::String) => Ok(prost_reflect::Value::String(
            String::from_utf8_lossy(&b).into_owned(),
        )),
        (Value::Bytes(b), Kind::Enum(descriptor)) => {
            let string = String::from_utf8_lossy(&b).into_owned();
            if let Some(d) = descriptor
                .values()
                .find(|v| v.name().eq_ignore_ascii_case(&string))
            {
                Ok(prost_reflect::Value::EnumNumber(d.number()))
            } else {
                Err(format!(
                    "Enum `{}` has no value that matches string '{}'",
                    descriptor.full_name(),
                    string
                ))
            }
        }
        (Value::Float(f), Kind::Double) => Ok(prost_reflect::Value::F64(f.into_inner())),
        (Value::Float(f), Kind::Float) => Ok(prost_reflect::Value::F32(f.into_inner() as f32)),
        (Value::Integer(i), Kind::Int32) => Ok(prost_reflect::Value::I32(i as i32)),
        (Value::Integer(i), Kind::Int64) => Ok(prost_reflect::Value::I64(i)),
        (Value::Integer(i), Kind::Sint32) => Ok(prost_reflect::Value::I32(i as i32)),
        (Value::Integer(i), Kind::Sint64) => Ok(prost_reflect::Value::I64(i)),
        (Value::Integer(i), Kind::Sfixed32) => Ok(prost_reflect::Value::I32(i as i32)),
        (Value::Integer(i), Kind::Sfixed64) => Ok(prost_reflect::Value::I64(i)),
        (Value::Integer(i), Kind::Uint32) => Ok(prost_reflect::Value::U32(i as u32)),
        (Value::Integer(i), Kind::Uint64) => Ok(prost_reflect::Value::U64(i as u64)),
        (Value::Integer(i), Kind::Fixed32) => Ok(prost_reflect::Value::U32(i as u32)),
        (Value::Integer(i), Kind::Fixed64) => Ok(prost_reflect::Value::U64(i as u64)),
        (Value::Integer(i), Kind::Enum(_)) => Ok(prost_reflect::Value::EnumNumber(i as i32)),
        (Value::Object(o), Kind::Message(message_descriptor)) => {
            if message_descriptor.is_map_entry() {
                let value_field = message_descriptor
                    .get_field_by_name("value")
                    .ok_or("Internal error with proto map processing")?;
                let mut map: HashMap<MapKey, prost_reflect::Value> = HashMap::new();
                for (key, val) in o.into_iter() {
                    match convert_value(&value_field, val) {
                        Ok(prost_val) => {
                            map.insert(MapKey::String(key.into()), prost_val);
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(prost_reflect::Value::Map(map))
            } else {
                // if it's not a map, it's an actual message
                Ok(prost_reflect::Value::Message(encode_message(
                    message_descriptor,
                    Value::Object(o),
                )?))
            }
        }
        (Value::Regex(r), Kind::String) => Ok(prost_reflect::Value::String(r.as_str().to_owned())),
        (Value::Regex(r), Kind::Bytes) => Ok(prost_reflect::Value::Bytes(r.as_bytes())),
        (Value::Timestamp(t), Kind::Int64) => Ok(prost_reflect::Value::I64(t.timestamp_micros())),
        (Value::Timestamp(t), Kind::Message(descriptor))
            if descriptor.full_name() == "google.protobuf.Timestamp" =>
        {
            let mut message = DynamicMessage::new(descriptor.clone());
            message
                .try_set_field_by_name("seconds", prost_reflect::Value::I64(t.timestamp()))
                .map_err(|e| format!("Error setting 'seconds' field: {}", e))?;
            message
                .try_set_field_by_name("nanos", prost_reflect::Value::I32(t.nanosecond() as i32))
                .map_err(|e| format!("Error setting 'nanos' field: {}", e))?;
            Ok(prost_reflect::Value::Message(message))
        }
        _ => Err(format!(
            "Cannot encode `{kind_str}` into protobuf `{kind:?}`",
        )),
    }
}

/// Convert a `Value` into a protobuf `Value`.
fn convert_value(
    field_descriptor: &FieldDescriptor,
    value: Value,
) -> std::result::Result<prost_reflect::Value, String> {
    if let Value::Array(a) = value {
        if field_descriptor.cardinality() == prost_reflect::Cardinality::Repeated {
            let repeated: std::result::Result<Vec<prost_reflect::Value>, String> = a
                .into_iter()
                .map(|v| convert_value_raw(v, &field_descriptor.kind()))
                .collect();
            Ok(prost_reflect::Value::List(repeated?))
        } else {
            Err("Cannot encode array into a non-repeated protobuf field".into())
        }
    } else {
        convert_value_raw(value, &field_descriptor.kind())
    }
}

/// Convert a `Value` into a protobuf message.
///
/// This function can only operate on `Value::Object`s,
/// since they are the only field-based Value
/// and protobuf messages are defined as a collection of fields and values.
pub fn encode_message(
    message_descriptor: &MessageDescriptor,
    value: Value,
) -> std::result::Result<DynamicMessage, String> {
    let mut message = DynamicMessage::new(message_descriptor.clone());
    if let Value::Object(map) = value {
        for field in message_descriptor.fields() {
            match map.get(field.name()) {
                None | Some(Value::Null) => message.clear_field(&field),
                Some(value) => message
                    .try_set_field(&field, convert_value(&field, value.clone())?)
                    .map_err(|e| format!("Error setting {} field: {}", field.name(), e))?,
            }
        }
        Ok(message)
    } else {
        Err("ProtobufSerializer only supports serializing objects".into())
    }
}

pub(crate) fn encode_proto(descriptor: &MessageDescriptor, value: Value) -> Resolved {
    let message = encode_message(descriptor, value)?;
    let mut buf = Vec::new();
    message
        .encode(&mut buf)
        .map_err(|e| format!("Error encoding protobuf message: {}", e))?;
    Ok(Value::Bytes(Bytes::from(buf)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobuf::get_message_descriptor;
    use crate::protobuf::parse_proto;
    use crate::value;
    use bytes::Bytes;
    use chrono::DateTime;
    use ordered_float::NotNan;
    use prost_reflect::MapKey;
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;
    use std::{env, fs};

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("tests/data/protobuf")
    }

    macro_rules! mfield {
        ($m:expr, $f:expr) => {
            $m.get_field_by_name($f).unwrap().into_owned()
        };
    }

    fn test_message_descriptor(message_type: &str) -> MessageDescriptor {
        let path = test_data_dir().join("test.desc");
        get_message_descriptor(&path, &format!("test.{message_type}")).unwrap()
    }

    #[test]
    fn test_encode_integers() {
        let message = encode_message(
            &test_message_descriptor("Integers"),
            Value::Object(BTreeMap::from([
                ("i32".into(), Value::Integer(-1234)),
                ("i64".into(), Value::Integer(-9876)),
                ("u32".into(), Value::Integer(1234)),
                ("u64".into(), Value::Integer(9876)),
            ])),
        )
        .unwrap();
        assert_eq!(Some(-1234), mfield!(message, "i32").as_i32());
        assert_eq!(Some(-9876), mfield!(message, "i64").as_i64());
        assert_eq!(Some(1234), mfield!(message, "u32").as_u32());
        assert_eq!(Some(9876), mfield!(message, "u64").as_u64());
    }

    #[test]
    fn test_encode_floats() {
        let message = encode_message(
            &test_message_descriptor("Floats"),
            Value::Object(BTreeMap::from([
                ("d".into(), Value::Float(NotNan::new(11.0).unwrap())),
                ("f".into(), Value::Float(NotNan::new(2.0).unwrap())),
            ])),
        )
        .unwrap();
        assert_eq!(Some(11.0), mfield!(message, "d").as_f64());
        assert_eq!(Some(2.0), mfield!(message, "f").as_f32());
    }

    #[test]
    fn test_encode_bytes() {
        let bytes = Bytes::from(vec![0, 1, 2, 3]);
        let message = encode_message(
            &test_message_descriptor("Bytes"),
            Value::Object(BTreeMap::from([
                ("text".into(), Value::Bytes(Bytes::from("vector"))),
                ("binary".into(), Value::Bytes(bytes.clone())),
            ])),
        )
        .unwrap();
        assert_eq!(Some("vector"), mfield!(message, "text").as_str());
        assert_eq!(Some(&bytes), mfield!(message, "binary").as_bytes());
    }

    #[test]
    fn test_encode_map() {
        let message = encode_message(
            &test_message_descriptor("Map"),
            Value::Object(BTreeMap::from([
                (
                    "names".into(),
                    Value::Object(BTreeMap::from([
                        ("forty-four".into(), Value::Integer(44)),
                        ("one".into(), Value::Integer(1)),
                    ])),
                ),
                (
                    "people".into(),
                    Value::Object(BTreeMap::from([(
                        "mark".into(),
                        Value::Object(BTreeMap::from([
                            ("nickname".into(), Value::Bytes(Bytes::from("jeff"))),
                            ("age".into(), Value::Integer(22)),
                        ])),
                    )])),
                ),
            ])),
        )
        .unwrap();
        // the simpler string->primitive map
        assert_eq!(
            Some(&HashMap::from([
                (
                    MapKey::String("forty-four".into()),
                    prost_reflect::Value::I32(44),
                ),
                (MapKey::String("one".into()), prost_reflect::Value::I32(1),),
            ])),
            mfield!(message, "names").as_map()
        );
        // the not-simpler string->message map
        let people = mfield!(message, "people").as_map().unwrap().to_owned();
        assert_eq!(1, people.len());
        assert_eq!(
            Some("jeff"),
            mfield!(
                people[&MapKey::String("mark".into())].as_message().unwrap(),
                "nickname"
            )
            .as_str()
        );
        assert_eq!(
            Some(22),
            mfield!(
                people[&MapKey::String("mark".into())].as_message().unwrap(),
                "age"
            )
            .as_u32()
        );
    }

    #[test]
    fn test_encode_enum() {
        let message = encode_message(
            &test_message_descriptor("Enum"),
            Value::Object(BTreeMap::from([
                ("breakfast".into(), Value::Bytes(Bytes::from("tomato"))),
                ("dinner".into(), Value::Bytes(Bytes::from("OLIVE"))),
                ("lunch".into(), Value::Integer(0)),
            ])),
        )
        .unwrap();
        assert_eq!(Some(2), mfield!(message, "breakfast").as_enum_number());
        assert_eq!(Some(0), mfield!(message, "lunch").as_enum_number());
        assert_eq!(Some(1), mfield!(message, "dinner").as_enum_number());
    }

    #[test]
    fn test_encode_timestamp() {
        let message = encode_message(
            &test_message_descriptor("Timestamp"),
            Value::Object(BTreeMap::from([(
                "morning".into(),
                Value::Timestamp(
                    DateTime::from_timestamp(8675, 309).expect("could not compute timestamp"),
                ),
            )])),
        )
        .unwrap();
        let timestamp = mfield!(message, "morning").as_message().unwrap().clone();
        assert_eq!(Some(8675), mfield!(timestamp, "seconds").as_i64());
        assert_eq!(Some(309), mfield!(timestamp, "nanos").as_i32());
    }

    #[test]
    fn test_encode_repeated_primitive() {
        let message = encode_message(
            &test_message_descriptor("RepeatedPrimitive"),
            Value::Object(BTreeMap::from([(
                "numbers".into(),
                Value::Array(vec![
                    Value::Integer(8),
                    Value::Integer(6),
                    Value::Integer(4),
                ]),
            )])),
        )
        .unwrap();
        let list = mfield!(message, "numbers").as_list().unwrap().to_vec();
        assert_eq!(3, list.len());
        assert_eq!(Some(8), list[0].as_i64());
        assert_eq!(Some(6), list[1].as_i64());
        assert_eq!(Some(4), list[2].as_i64());
    }

    #[test]
    fn test_encode_repeated_message() {
        let message = encode_message(
            &test_message_descriptor("RepeatedMessage"),
            Value::Object(BTreeMap::from([(
                "messages".into(),
                Value::Array(vec![
                    Value::Object(BTreeMap::from([(
                        "text".into(),
                        Value::Bytes(Bytes::from("vector")),
                    )])),
                    Value::Object(BTreeMap::from([("index".into(), Value::Integer(4444))])),
                    Value::Object(BTreeMap::from([
                        ("text".into(), Value::Bytes(Bytes::from("protobuf"))),
                        ("index".into(), Value::Integer(1)),
                    ])),
                ]),
            )])),
        )
        .unwrap();
        let list = mfield!(message, "messages").as_list().unwrap().to_vec();
        assert_eq!(3, list.len());
        assert_eq!(
            Some("vector"),
            mfield!(list[0].as_message().unwrap(), "text").as_str()
        );
        assert!(!list[0].as_message().unwrap().has_field_by_name("index"));
        assert!(!list[1].as_message().unwrap().has_field_by_name("t4ext"));
        assert_eq!(
            Some(4444),
            mfield!(list[1].as_message().unwrap(), "index").as_u32()
        );
        assert_eq!(
            Some("protobuf"),
            mfield!(list[2].as_message().unwrap(), "text").as_str()
        );
        assert_eq!(
            Some(1),
            mfield!(list[2].as_message().unwrap(), "index").as_u32()
        );
    }

    fn read_pb_file(protobuf_bin_message_path: &str) -> String {
        fs::read_to_string(test_data_dir().join(protobuf_bin_message_path)).unwrap()
    }

    #[test]
    fn test_parse_files() {
        let value = value!({ name: "someone", phones: [{number: "123456"}] });
        let path = test_data_dir().join("test_protobuf.desc");
        let descriptor = get_message_descriptor(&path, "test_protobuf.Person").unwrap();
        let expected_value = value!(read_pb_file("person_someone.pb"));
        let encoded_value = encode_proto(&descriptor, value.clone());
        assert!(
            encoded_value.is_ok(),
            "Failed to encode proto: {:?}",
            encoded_value.unwrap_err()
        ); // Check if the Result is Ok
        let encoded_value = encoded_value.unwrap();
        assert_eq!(expected_value.as_bytes(), encoded_value.as_bytes());

        // Also test parse_proto.
        let parsed_value = parse_proto(&descriptor, encoded_value);
        assert!(
            parsed_value.is_ok(),
            "Failed to parse proto: {:?}",
            parsed_value.unwrap_err()
        );
        let parsed_value = parsed_value.unwrap();
        assert_eq!(value, parsed_value)
    }

    #[test]
    fn test_parse_proto3() {
        let value = value!({ data: {data_phone: "HOME"}, name: "someone", phones: [{number: "1234", type: "MOBILE"}] });
        let path = test_data_dir().join("test_protobuf3.desc");
        let descriptor = get_message_descriptor(&path, "test_protobuf3.Person").unwrap();
        let expected_value = value!(read_pb_file("person_someone3.pb"));
        let encoded_value = encode_proto(&descriptor, value.clone());
        assert!(
            encoded_value.is_ok(),
            "Failed to encode proto: {:?}",
            encoded_value.unwrap_err()
        ); // Check if the Result is Ok
        let encoded_value = encoded_value.unwrap();
        assert_eq!(expected_value.as_bytes(), encoded_value.as_bytes());

        // Also test parse_proto.
        let parsed_value = parse_proto(&descriptor, encoded_value);
        assert!(
            parsed_value.is_ok(),
            "Failed to parse proto: {:?}",
            parsed_value.unwrap_err()
        );
        let parsed_value = parsed_value.unwrap();
        assert_eq!(value, parsed_value)
    }
}
