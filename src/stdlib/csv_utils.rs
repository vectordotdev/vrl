use crate::{compiler::prelude::*, prelude::Value};

pub(crate) fn parse_single_byte_delimiter(delimiter: Value) -> Result<u8, ExpressionError> {
    let delimiter = delimiter.try_bytes()?;

    if delimiter.len() == 1 {
        Ok(delimiter[0])
    } else {
        Err("delimiter must be a single character".into())
    }
}
