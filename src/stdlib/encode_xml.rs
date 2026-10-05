use crate::compiler::prelude::*;

const PARAMETERS: &[Parameter] = &[Parameter::required(
    "value",
    kind::OBJECT,
    "The object containing one root element. Keys prefixed with `@` become attributes; the `text` key emits text content.",
)];

fn encode_xml(value: Value) -> Resolved {
    let object = value.try_object().map_err(|_| {
        ExpressionError::from("value must be an object with exactly one root element")
    })?;
    if object.len() != 1 {
        return Err(ExpressionError::from(
            "value must be an object with exactly one root element",
        ));
    }

    let (root, value) = object.into_iter().next().expect("one root element");
    let mut output = String::new();
    write_element(&mut output, root.as_ref(), &value)?;

    // Validate element names, attributes, and XML 1.0 characters before returning the output.
    roxmltree::Document::parse(&output)
        .map_err(|error| ExpressionError::from(format!("unable to encode XML: {error}")))?;

    Ok(output.into())
}

fn write_element(output: &mut String, name: &str, value: &Value) -> Result<(), String> {
    if matches!(value, Value::Array(_)) {
        return Err(
            "XML elements cannot have array values; use arrays for repeated child elements"
                .to_owned(),
        );
    }

    output.push('<');
    output.push_str(name);

    let object = match value {
        Value::Object(object) => Some(object),
        _ => None,
    };

    if let Some(object) = object {
        for (key, value) in object.iter().filter(|(key, _)| key.starts_with('@')) {
            let attribute = &key[1..];
            output.push(' ');
            output.push_str(attribute);
            output.push_str("=\"");
            escape_xml(&value_to_string(value)?, output, true);
            output.push('"');
        }
    }

    let has_children = match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            !key.starts_with('@') && !matches!(value, Value::Array(values) if values.is_empty())
        }),
        Value::Array(values) => !values.is_empty(),
        Value::Null => false,
        _ => true,
    };

    if !has_children {
        output.push_str("/>");
        return Ok(());
    }

    output.push('>');
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter().filter(|(key, _)| !key.starts_with('@')) {
                if key.as_ref() == "text" {
                    write_text_value(value, output)?;
                } else {
                    write_named_value(key.as_ref(), value, output)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                write_text_value(value, output)?;
            }
        }
        Value::Null => {}
        value => escape_xml(&value_to_string(value)?, output, false),
    }
    output.push_str("</");
    output.push_str(name);
    output.push('>');
    Ok(())
}

fn write_named_value(name: &str, value: &Value, output: &mut String) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                write_element(output, name, value)?;
            }
            Ok(())
        }
        value => write_element(output, name, value),
    }
}

fn write_text_value(value: &Value, output: &mut String) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                write_text_value(value, output)?;
            }
            Ok(())
        }
        Value::Object(_) => Err("XML text values must be scalar".to_owned()),
        value => {
            escape_xml(&value_to_string(value)?, output, false);
            Ok(())
        }
    }
}

fn value_to_string(value: &Value) -> Result<String, String> {
    if matches!(value, Value::Object(_) | Value::Array(_)) {
        return Err("XML attributes and text values must be scalar".to_owned());
    }
    let bytes = value.coerce_to_bytes();
    String::from_utf8(bytes.to_vec()).map_err(|_| "XML values must contain valid UTF-8".to_owned())
}

fn escape_xml(value: &str, output: &mut String, attribute: bool) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attribute => output.push_str("&quot;"),
            '\t' if attribute => output.push_str("&#9;"),
            '\n' if attribute => output.push_str("&#10;"),
            '\r' if attribute => output.push_str("&#13;"),
            '\r' => output.push_str("&#13;"),
            character => output.push(character),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeXml;

impl Function for EncodeXml {
    fn identifier(&self) -> &'static str {
        "encode_xml"
    }

    fn usage(&self) -> &'static str {
        "Encodes a single-root object as XML, using `@`-prefixed keys for attributes and the `text` key for text content."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` must be an object with exactly one root element.",
            "XML element names and values must form a valid XML 1.0 document.",
            "XML text values must be scalar and contain valid UTF-8.",
            "XML elements cannot have array values; use arrays for repeated child elements.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        Ok(EncodeXmlFn {
            value: arguments.required("value"),
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Encode an object as XML",
            source: r#"encode_xml!({"book": {"@category": "fiction", "title": "Dune"}})"#,
            result: Ok(r#"<book category="fiction"><title>Dune</title></book>"#),
        }]
    }
}

#[derive(Clone, Debug)]
struct EncodeXmlFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeXmlFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        encode_xml(self.value.resolve(ctx)?)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parsing::xml::ParseOptions, value};

    test_function![
        encode_xml => EncodeXml;

        nested_object {
            args: func_args![value: value!({"book": {"@category": "fiction", "title": "Dune"}})],
            want: Ok(r#"<book category="fiction"><title>Dune</title></book>"#),
            tdef: TypeDef::bytes().fallible(),
        }

        repeated_elements {
            args: func_args![value: value!({"root": {"item": ["one", "two"]}})],
            want: Ok("<root><item>one</item><item>two</item></root>"),
            tdef: TypeDef::bytes().fallible(),
        }

        empty_array {
            args: func_args![value: value!({"root": {"item": []}})],
            want: Ok("<root/>"),
            tdef: TypeDef::bytes().fallible(),
        }

        escaped_text_and_attribute {
            args: func_args![value: value!({"root": {"@name": "A & B", "text": "<hello>"}})],
            want: Ok(r#"<root name="A &amp; B">&lt;hello&gt;</root>"#),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_root_count {
            args: func_args![value: value!({"one": 1, "two": 2})],
            want: Err("value must be an object with exactly one root element"),
            tdef: TypeDef::bytes().fallible(),
        }

    ];

    #[test]
    fn invalid_element_name_is_rejected() {
        let error = encode_xml(value!({"not an element": "value"})).unwrap_err();
        assert!(error.to_string().starts_with("unable to encode XML:"));
    }

    #[test]
    fn xml_encoding_round_trips_with_parse_xml() {
        let input = value!({
            "root": {
                "@id": "42",
                "child": ["first", "second"],
            }
        });
        let encoded = encode_xml(input.clone()).unwrap();
        let decoded = crate::parsing::xml::parse_xml(encoded, ParseOptions::default()).unwrap();
        assert_eq!(decoded, input);
    }
}
