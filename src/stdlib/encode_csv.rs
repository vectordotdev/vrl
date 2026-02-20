use {crate::compiler::prelude::*, csv::WriterBuilder, std::sync::LazyLock};

static DEFAULT_DELIMITER: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from(",")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required("value", kind::ANY, "The value to convert to a CSV string."),
        Parameter::optional(
            "delimiter",
            kind::BYTES,
            "The field delimiter to use when encoding. Must be a single-byte utf8 character.",
        )
        .default(&DEFAULT_DELIMITER),
    ]
});

// TODO: armand we may need to include it in the benchs? I dont know what this is lol
fn encode_csv(value: Value, delimiter: Value) -> Resolved {
    let value_array = value
        .try_array()?
        .into_iter()
        .map(VrlValueConvert::try_bytes)
        .collect::<Result<Vec<Bytes>, ValueError>>()?;

    // When empty array, return empty string directly.
    // The csv crate writes an empty record as "" which is valid CSV, but we want empty arrays to
    // produce empty strings.
    if value_array.is_empty() {
        return Ok(Value::Bytes(Bytes::from("")));
    }

    // TODO: armand this code exists as well in https://github.com/armleth/vrl/blob/f62458e8d0a0bd9ce941bab61cf0ee5a49391a46/src/stdlib/parse_csv.rs#L21-L24. May need a helper function.
    let delimiter = delimiter.try_bytes()?;
    if delimiter.len() != 1 {
        return Err("delimiter must be a single character".into());
    }
    let delimiter = delimiter[0];

    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_writer(vec![]);

    writer
        .write_record(&value_array)
        .map_err(|err| format!("unable to encode to csv: {err}"))?;

    let mut result = writer
        .into_inner()
        .map_err(|err| format!("unable to encode to csv: {err}"))?;

    // As we handle only one-line CSVs, a line terminator is never required.
    // Since the csv crate's WriterBuilder does not allow disabling the terminator,
    // we must remove it manually here.
    result.pop();

    Ok(Value::Bytes(Bytes::from(result)))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeCsv;

impl Function for EncodeCsv {
    fn identifier(&self) -> &'static str {
        "encode_csv"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to CSV. Line breaks are escaped."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "The delimiter must be a single-byte UTF-8 character.",
            "`value` is not an object convertible to a CSV string.",
            "The `csv` crate encountered an I/O error while writing or flushing the output.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Encode object to a single CSV formatted row",
                source: r#"encode_csv!(["foo","bar","foo \", bar"])"#,
                result: Ok(
                    r#"
                        s'foo,bar,\"foo \"\", bar\"'
                    "#
                )
            },
            example! {
                title: "Encode object to a single CSV formatted row with custom delimiter",
                source: r#"encode_csv!(["foo","bar"], delimiter: " ")"#,
                result: Ok(r#""foo bar""#)
            },
            example! {
                title: "Encode object to a single CSV formatted row with linebreaks",
                source: r#"encode_csv!(["line", "with_linebreak", "here\n", "and", "\nhere"])"#,
                result: Ok(
                    r#"
                        s'line,with_linebreak,\"here\n\",and,\"\nhere\"'
                    "#
                )
            },
        ]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let delimiter = arguments.optional("delimiter");

        Ok(EncodeCsvFn { value, delimiter }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }
}

#[derive(Clone, Debug)]
struct EncodeCsvFn {
    value: Box<dyn Expression>,
    delimiter: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeCsvFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self
            .value
            .resolve(ctx)?;

        let delimiter = self
            .delimiter
            .map_resolve_with_default(ctx, || DEFAULT_DELIMITER.clone())?;

        encode_csv(value, delimiter)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::value};

    test_function![
        parse_csv => EncodeCsv;

        valid {
            args: func_args![value: value!(["foo", "bar", "foo \", bar"])],
            want: Ok(value!("foo,bar,\"foo \"\", bar\"")),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_utf8 {
            args: func_args![value: value!(vec!["foo".into(), value!(Bytes::copy_from_slice(&b"b\xFFar"[..]))])],
            want: Ok(value!(Bytes::copy_from_slice(&b"foo,b\xFFar"[..]))),
            tdef: TypeDef::bytes().fallible(),
        }

        custom_delimiter {
            args: func_args![value: value!(["foo", "bar"]), delimiter: value!(" ")],
            want: Ok(value!("foo bar")),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_delimiter {
            args: func_args![value: value!(["foo", "bar"]), delimiter: value!("!!")],
            want: Err("delimiter must be a single character"),
            tdef: TypeDef::bytes().fallible(),
        }

        single_value {
            args: func_args![value: value!(["foo"])],
            want: Ok(value!("foo")),
            tdef: TypeDef::bytes().fallible(),
        }

        empty_string {
            args: func_args![value: value!([])],
            want: Ok(value!("")),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_lines {
            args: func_args![value: value!(["line", "with_linebreak", "here\n", "and", "\nhere"])],
            want: Ok(value!("line,with_linebreak,\"here\n\",and,\"\nhere\"")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
