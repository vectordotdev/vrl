use crate::compiler::ExpressionError;
use crate::prelude::{VrlValueConvert, value::Value};
use std::convert::TryInto;

pub(crate) fn to_key<const N: usize>(
    key: Value,
    mode: &str,
    ip_ver: &str,
) -> Result<[u8; N], ExpressionError> {
    let key_bytes = key.try_bytes()?;
    let msg = format!("{mode} mode requires a {N}-byte key for {ip_ver}");
    if key_bytes.len() != N {
        return Err(msg.into());
    }
    key_bytes.as_ref().try_into().map_err(|_| msg.into())
}
