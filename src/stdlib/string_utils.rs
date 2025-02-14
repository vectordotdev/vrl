use crate::prelude::{Value, ValueError, VrlValueConvert};

pub(crate) fn convert_to_string(value: &Value, to_lowercase: bool) -> Result<String, ValueError> {
    let string = value.try_bytes_utf8_lossy()?;
    Ok(if to_lowercase {
        string.to_lowercase()
    } else {
        string.to_string()
    })
}
