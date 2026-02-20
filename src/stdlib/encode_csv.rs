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

// TODO: armand do we include a `pretty` bool parameter? like in encode_json?
// TODO: armand we may need to include it in the benchs? I dont know what this is lol
// TODO: armand for now, we always insert an \n at the end of the csv
fn encode_csv(value: Value, delimiter: Value) -> Resolved {
    // TODO: armand we need to have an array to be able to pass it to writerBuilder. However, it
    // gives us two things to think about:
    // - with this implementation we have a kind of "black box": we not only handle encoding to csv
    // for csv object, but for every object that is parsed into a Vec<Value>. We need to find a way
    // to write this in the documentation (if thif is the expected comportment).
    // - gives us another way to fail at runtime
    //
    //
    // TODO: armand i have a redundant_closure_for_method_calls
    // but can't remove it bc the function is in a private module.
    // https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#redundant_closure_for_method_calls
    let value_array = value
        .try_array()?
        .into_iter()
        .map(|array_element| array_element.try_bytes())
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
        .terminator(csv::Terminator::Any(b'\0'))
        .from_writer(vec![]);

    // TODO: armand investigate what are the cases where the two following blocks can fail.
    writer
        .write_record(&value_array)
        .map_err(|err| format!("unable to encode to csv: {err}"))?;

    let result = writer
        .into_inner()
        .map_err(|err| format!("unable to encode to csv: {err}"))?;

    Ok(Value::Bytes(Bytes::from(result)))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeCsv;

// TODO: armand check if i implemented every needed method
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

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        todo!()
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

        // TODO: armand there might be a way to avoid copying the default delimiter if i dont use
        // the map_resolve_with_default helper function.
        let delimiter = self
            .delimiter
            .map_resolve_with_default(ctx, || DEFAULT_DELIMITER.clone())?;

        encode_csv(value, delimiter)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        // TODO: armand i can't think about cases where this might fail. Have to check with the doc
        TypeDef::bytes().fallible()
    }
}
