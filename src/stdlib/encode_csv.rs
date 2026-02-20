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
// TODO: armand how are we handling multiple lines inputs? For now, it just put the \n as a normal
// character
// TODO: armand empty input => giving us "\"\"" where it should be ""
fn encode_csv(value: Value, delimiter: Value) -> Resolved {
    let value_array = value
        .try_array()?
        .into_iter()
        .map(VrlValueConvert::try_bytes)
        .collect::<Result<Vec<Bytes>, ValueError>>()?;

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
        "Encodes the `value` to CSV."
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
                title: "Encode object to a single CSV formatted row with custom delimiter ",
                source: r#"encode_csv!(["foo","bar"], delimiter: " ")"#,
                result: Ok(r#""foo bar""#)
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
