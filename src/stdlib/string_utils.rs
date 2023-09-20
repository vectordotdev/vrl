use crate::prelude::{Value, ValueError, VrlValueConvert};

pub(crate) fn convert_to_string(value: Value, case_sensitive: bool) -> Result<String, ValueError> {
    let string = value.try_bytes_utf8_lossy()?;
    Ok(match case_sensitive {
        true => string.to_string(),
        false => string.to_lowercase(),
    })
}
