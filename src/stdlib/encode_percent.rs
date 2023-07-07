use crate::compiler::prelude::*;
use crate::value;
use percent_encoding::{utf8_percent_encode, AsciiSet};

fn encode_percent(value: Value, ascii_set: &Bytes) -> Resolved {
    let string = value.try_bytes_utf8_lossy()?;
    let ascii_set = match ascii_set.as_ref() {
        b"NON_ALPHANUMERIC" => percent_encoding::NON_ALPHANUMERIC,
        b"CONTROLS" => percent_encoding::CONTROLS,
        b"FRAGMENT" => FRAGMENT,
        b"QUERY" => QUERY,
        b"SPECIAL" => SPECIAL,
        b"PATH" => PATH,
        b"USERINFO" => USERINFO,
        b"COMPONENT" => COMPONENT,
        b"WWW_FORM_URLENCODED" => WWW_FORM_URLENCODED,
        _ => unreachable!("enum invariant"),
    };

    Ok(utf8_percent_encode(&string, ascii_set).to_string().into())
}

/// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT: &AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`');

/// https://url.spec.whatwg.org/#query-percent-encode-set
const QUERY: &AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>');

/// https://url.spec.whatwg.org/#special-percent-encode-set
const SPECIAL: &AsciiSet = &QUERY.add(b'\'');

/// https://url.spec.whatwg.org/#path-percent-encode-set
const PATH: &AsciiSet = &QUERY.add(b'?').add(b'`').add(b'{').add(b'}');

/// https://url.spec.whatwg.org/#userinfo-percent-encode-set
const USERINFO: &AsciiSet = &PATH
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'|');

/// https://url.spec.whatwg.org/#component-percent-encode-set
const COMPONENT: &AsciiSet = &USERINFO.add(b'$').add(b'%').add(b'&').add(b'+').add(b',');

/// https://url.spec.whatwg.org/#application-x-www-form-urlencoded-percent-encode-set
const WWW_FORM_URLENCODED: &AsciiSet =
    &COMPONENT.add(b'!').add(b'\'').add(b'(').add(b')').add(b'~');

#[derive(Clone, Copy, Debug)]
pub struct EncodePercent;

fn ascii_sets() -> Vec<Value> {
    vec![
        value!("NON_ALPHANUMERIC"),
        value!("CONTROLS"),
        value!("FRAGMENT"),
        value!("QUERY"),
        value!("SPECIAL"),
        value!("PATH"),
        value!("USERINFO"),
        value!("COMPONENT"),
        value!("WWW_FORM_URLENCODED"),
    ]
}

impl Function for EncodePercent {
    fn identifier(&self) -> &'static str {
        "encode_percent"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "ascii_set",
                kind: kind::BYTES,
                required: false,
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
        let ascii_set = arguments
            .optional_enum("ascii_set", &ascii_sets(), state)?
            .unwrap_or_else(|| value!("NON_ALPHANUMERIC"))
            .try_bytes()
            .expect("ascii_set not bytes");

        Ok(EncodePercentFn { value, ascii_set }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "percent encode string",
                source: r#"encode_percent("foo bar?")"#,
                result: Ok(r#"s'foo%20bar%3F'"#),
            },
            Example {
                title: "percent encode for query",
                source: r#"encode_percent("foo@bar?")"#,
                result: Ok(r#"s'foo%40bar%3F'"#),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodePercentFn {
    value: Box<dyn Expression>,
    ascii_set: Bytes,
}

impl FunctionExpression for EncodePercentFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        encode_percent(value, &self.ascii_set)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        encode_percent => EncodePercent;

        default {
            args: func_args![value: r#"foo bar?"#],
            want: Ok(r#"foo%20bar%3F"#),
            tdef: TypeDef::bytes().infallible(),
        }

        controls {
            args: func_args![value: r#"foo bar"#, ascii_set: "CONTROLS"],
            want: Ok(r#"foo %14bar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        fragment {
            args: func_args![value: r#"foo <>" `bar"#, ascii_set: "FRAGMENT"],
            want: Ok(r#"foo%20%3C%3E%22%20%60bar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        query {
            args: func_args![value: r#"foo #"<>bar"#, ascii_set: "QUERY"],
            want: Ok(r#"foo%20%23%22%3C%3Ebar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        special {
            args: func_args![value: r#"foo #"<>'bar"#, ascii_set: "SPECIAL"],
            want: Ok(r#"foo%20%23%22%3C%3E%27bar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        path {
            args: func_args![value: r#"foo #"<>?`{}bar"#, ascii_set: "PATH"],
            want: Ok(r#"foo%20%23%22%3C%3E%3F%60%7B%7Dbar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        userinfo {
            args: func_args![value: r#"foo #"<>?`{}/:;=@[\]^|bar"#, ascii_set: "USERINFO"],
            want: Ok(r#"foo%20%23%22%3C%3E%3F%60%7B%7D%2F%3A%3B%3D%40%5B%5C%5D%5E%7Cbar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        component {
            args: func_args![value: r#"foo #"<>?`{}/:;=@[\]^|$%&+,bar"#, ascii_set: "COMPONENT"],
            want: Ok(r#"foo%20%23%22%3C%3E%3F%60%7B%7D%2F%3A%3B%3D%40%5B%5C%5D%5E%7C%24%25%26%2B%2Cbar"#),
            tdef: TypeDef::bytes().infallible(),
        }

        www_form_urlencoded {
            args: func_args![value: r#"foo #"<>?`{}/:;=@[\]^|$%&+,!'()~bar"#, ascii_set: "WWW_FORM_URLENCODED"],
            want: Ok(r#"foo%20%23%22%3C%3E%3F%60%7B%7D%2F%3A%3B%3D%40%5B%5C%5D%5E%7C%24%25%26%2B%2C%21%27%28%29%7Ebar"#),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
