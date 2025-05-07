use crate::prelude::{Value, ValueError, VrlValueConvert};
use std::borrow::Cow;

pub(crate) fn convert_to_string(value: &Value, to_lowercase: bool) -> Result<Cow<str>, ValueError> {
    let string = value.try_bytes_utf8_lossy()?;
    Ok(if to_lowercase {
        Cow::Owned(string.to_lowercase())
    } else {
        string
    })
}
