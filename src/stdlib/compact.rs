use super::util;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_RECURSIVE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_NULL: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_STRING: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_OBJECT: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_ARRAY: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_NULLISH: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The object or array to compact.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "recursive",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether the compaction be recursive.",
            default: Some(&DEFAULT_RECURSIVE),
            enum_variants: None,
        },
        Parameter {
            keyword: "null",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether null should be treated as an empty value.",
            default: Some(&DEFAULT_NULL),
            enum_variants: None,
        },
        Parameter {
            keyword: "string",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether an empty string should be treated as an empty value.",
            default: Some(&DEFAULT_STRING),
            enum_variants: None,
        },
        Parameter {
            keyword: "object",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether an empty object should be treated as an empty value.",
            default: Some(&DEFAULT_OBJECT),
            enum_variants: None,
        },
        Parameter {
            keyword: "array",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether an empty array should be treated as an empty value.",
            default: Some(&DEFAULT_ARRAY),
            enum_variants: None,
        },
        Parameter {
            keyword: "nullish",
            kind: kind::BOOLEAN,
            required: false,
            description: "Tests whether the value is \"nullish\" as defined by the [`is_nullish`](#is_nullish) function.",
            default: Some(&DEFAULT_NULLISH),
            enum_variants: None,
        },
    ]
});

fn compact(
    recursive: Value,
    null: Value,
    string: Value,
    object: Value,
    array: Value,
    nullish: Value,
    value: Value,
) -> Resolved {
    let options = CompactOptions {
        recursive: recursive.try_boolean()?,
        null: null.try_boolean()?,
        string: string.try_boolean()?,
        object: object.try_boolean()?,
        array: array.try_boolean()?,
        nullish: nullish.try_boolean()?,
    };

    match value {
        Value::Object(object) => Ok(Value::from(compact_object(object, &options))),
        Value::Array(arr) => Ok(Value::from(compact_array(arr, &options))),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::array(Collection::any()) | Kind::object(Collection::any()),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Compact;

impl Function for Compact {
    fn identifier(&self) -> &'static str {
        "compact"
    }

    fn usage(&self) -> &'static str {
        "Compacts the `value` by removing empty values, where empty values are defined using the available parameters."
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY | kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &["The return type matches the `value` type."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Compact an object with default parameters",
                source: r#"compact({"field1": 1, "field2": "", "field3": [], "field4": null})"#,
                result: Ok(r#"{ "field1": 1 }"#),
            },
            example! {
                title: "Compact an array with default parameters",
                source: r#"compact(["foo", "bar", "", null, [], "buzz"])"#,
                result: Ok(r#"["foo","bar","buzz"]"#),
            },
            example! {
                title: "Compact an array using nullish",
                source: r#"compact(["-", "   ", "\n", null, true], nullish: true)"#,
                result: Ok("[true]"),
            },
            example! {
                title: "Compact a more complex object with default parameters",
                source: r#"compact({ "a": {}, "b": null, "c": [null], "d": "", "e": "-", "f": true })"#,
                result: Ok(r#"{ "e": "-", "f": true }"#),
            },
            example! {
                title: "Compact a more complex object using null: false",
                source: r#"compact({ "a": {}, "b": null, "c": [null], "d": "", "e": "-", "f": true }, null: false)"#,
                result: Ok(r#"{ "b": null, "c": [null], "e": "-", "f": true }"#),
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
        let recursive = arguments.optional("recursive");
        let null = arguments.optional("null");
        let string = arguments.optional("string");
        let object = arguments.optional("object");
        let array = arguments.optional("array");
        let nullish = arguments.optional("nullish");

        Ok(CompactFn {
            value,
            recursive,
            null,
            string,
            object,
            array,
            nullish,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct CompactFn {
    value: Box<dyn Expression>,
    recursive: Option<Box<dyn Expression>>,
    null: Option<Box<dyn Expression>>,
    string: Option<Box<dyn Expression>>,
    object: Option<Box<dyn Expression>>,
    array: Option<Box<dyn Expression>>,
    nullish: Option<Box<dyn Expression>>,
}

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // TODO replace with bitflags
struct CompactOptions {
    recursive: bool,
    null: bool,
    string: bool,
    object: bool,
    array: bool,
    nullish: bool,
}

impl Default for CompactOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            null: true,
            string: true,
            object: true,
            array: true,
            nullish: false,
        }
    }
}

impl CompactOptions {
    /// Check if the value is empty according to the given options
    fn is_empty(&self, value: &Value) -> bool {
        if self.nullish && util::is_nullish(value) {
            return true;
        }

        match value {
            Value::Bytes(bytes) => self.string && bytes.len() == 0,
            Value::Null => self.null,
            Value::Object(object) => self.object && object.is_empty(),
            Value::Array(array) => self.array && array.is_empty(),
            _ => false,
        }
    }
}

impl FunctionExpression for CompactFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let recursive = self
            .recursive
            .map_resolve_with_default(ctx, || DEFAULT_RECURSIVE.clone())?;
        let null = self
            .null
            .map_resolve_with_default(ctx, || DEFAULT_NULL.clone())?;
        let string = self
            .string
            .map_resolve_with_default(ctx, || DEFAULT_STRING.clone())?;
        let object = self
            .object
            .map_resolve_with_default(ctx, || DEFAULT_OBJECT.clone())?;
        let array = self
            .array
            .map_resolve_with_default(ctx, || DEFAULT_ARRAY.clone())?;
        let nullish = self
            .nullish
            .map_resolve_with_default(ctx, || DEFAULT_NULLISH.clone())?;
        let value = self.value.resolve(ctx)?;

        compact(recursive, null, string, object, array, nullish, value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        if self.value.type_def(state).is_array() {
            TypeDef::array(Collection::any())
        } else {
            TypeDef::object(Collection::any())
        }
    }
}

/// Compact the value if we are recursing - otherwise, just return the value untouched.
fn recurse_compact(value: Value, options: &CompactOptions) -> Value {
    match value {
        Value::Array(array) if options.recursive => Value::from(compact_array(array, options)),
        Value::Object(object) if options.recursive => Value::from(compact_object(object, options)),
        _ => value,
    }
}

fn compact_object(object: ObjectMap, options: &CompactOptions) -> ObjectMap {
    object
        .into_iter()
        .filter_map(|(key, value)| {
            let value = recurse_compact(value, options);
            if options.is_empty(&value) {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

fn compact_array(array: Vec<Value>, options: &CompactOptions) -> Vec<Value> {
    array
        .into_iter()
        .filter_map(|value| {
            let value = recurse_compact(value, options);
            if options.is_empty(&value) {
                None
            } else {
                Some(value)
            }
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::btreemap;

    #[test]
    fn test_compacted_array() {
        let cases = vec![
            (
                vec!["".into(), "".into()],              // expected
                vec!["".into(), Value::Null, "".into()], // original
                CompactOptions {
                    string: false,
                    ..CompactOptions::default()
                },
            ),
            (
                vec![1.into(), 2.into()],
                vec![1.into(), Value::Array(vec![]), 2.into()],
                CompactOptions::default(),
            ),
            (
                vec![1.into(), Value::Array(vec![3.into()]), 2.into()],
                vec![
                    1.into(),
                    Value::Array(vec![Value::Null, 3.into(), Value::Null]),
                    2.into(),
                ],
                CompactOptions::default(),
            ),
            (
                vec![1.into(), 2.into()],
                vec![
                    1.into(),
                    Value::Array(vec![Value::Null, Value::Null]),
                    2.into(),
                ],
                CompactOptions::default(),
            ),
            (
                vec![
                    Value::from(1),
                    Value::Object(ObjectMap::from([(
                        KeyString::from("field2"),
                        Value::from(2),
                    )])),
                    Value::from(2),
                ],
                vec![
                    1.into(),
                    Value::Object(ObjectMap::from([
                        (KeyString::from("field1"), Value::Null),
                        (KeyString::from("field2"), Value::from(2)),
                    ])),
                    2.into(),
                ],
                CompactOptions::default(),
            ),
        ];

        for (expected, original, options) in cases {
            assert_eq!(expected, compact_array(original, &options));
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compacted_map() {
        let cases = vec![
            (
                btreemap! {
                    "key1" => "",
                    "key3" => "",
                }, // expected
                btreemap! {
                    "key1" => "",
                    "key2" => Value::Null,
                    "key3" => "",
                }, // original
                CompactOptions {
                    string: false,
                    ..CompactOptions::default()
                },
            ),
            (
                btreemap! {
                    "key1" => Value::from(1),
                    "key3" => Value::from(2),
                },
                btreemap! {
                    "key1" => Value::from(1),
                    "key2" => Value::Array(vec![]),
                    "key3" => Value::from(2),
                },
                CompactOptions::default(),
            ),
            (
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (
                        KeyString::from("key2"),
                        Value::Object(ObjectMap::from([(KeyString::from("key2"), Value::from(3))])),
                    ),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (
                        KeyString::from("key2"),
                        Value::Object(ObjectMap::from([
                            (KeyString::from("key1"), Value::Null),
                            (KeyString::from("key2"), Value::from(3)),
                            (KeyString::from("key3"), Value::Null),
                        ])),
                    ),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                CompactOptions::default(),
            ),
            (
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (
                        KeyString::from("key2"),
                        Value::Object(ObjectMap::from([(KeyString::from("key1"), Value::Null)])),
                    ),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (
                        KeyString::from("key2"),
                        Value::Object(ObjectMap::from([(KeyString::from("key1"), Value::Null)])),
                    ),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                CompactOptions {
                    recursive: false,
                    ..CompactOptions::default()
                },
            ),
            (
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                ObjectMap::from([
                    (KeyString::from("key1"), Value::from(1)),
                    (
                        KeyString::from("key2"),
                        Value::Object(ObjectMap::from([(KeyString::from("key1"), Value::Null)])),
                    ),
                    (KeyString::from("key3"), Value::from(2)),
                ]),
                CompactOptions::default(),
            ),
            (
                btreemap! {
                    "key1" => Value::from(1),
                    "key2" => Value::Array(vec![2.into()]),
                    "key3" => Value::from(2),
                },
                btreemap! {
                    "key1" => Value::from(1),
                    "key2" => Value::Array(vec![Value::Null, 2.into(), Value::Null]),
                    "key3" => Value::from(2),
                },
                CompactOptions::default(),
            ),
        ];

        for (expected, original, options) in cases {
            assert_eq!(expected, compact_object(original, &options));
        }
    }

    test_function![
        compact => Compact;

        with_map {
            args: func_args![value: Value::from(ObjectMap::from([(KeyString::from("key1"), Value::Null), (KeyString::from("key2"), Value::from(1)), (KeyString::from("key3"), Value::from(""))]))],
            want: Ok(Value::Object(ObjectMap::from([(KeyString::from("key2"), Value::from(1))]))),
            tdef: TypeDef::object(Collection::any()),
        }

        with_array {
            args: func_args![value: vec![Value::Null, Value::from(1), Value::from(""),]],
            want: Ok(Value::Array(vec![Value::from(1)])),
            tdef: TypeDef::array(Collection::any()),
        }

        nullish {
            args: func_args![
                value: btreemap! {
                    "key1" => "-",
                    "key2" => 1,
                    "key3" => " "
                },
                nullish: true
            ],
            want: Ok(Value::Object(ObjectMap::from([(KeyString::from("key2"), Value::from(1))]))),
            tdef: TypeDef::object(Collection::any()),
        }
    ];
}
