/// Create a boxed [`Expression`][crate::Expression] trait object from a given `Value`.
///
/// Supports the same format as the [`value`] macro.
#[macro_export]
macro_rules! expr {
    ($($v:tt)*) => {{
        let value = $crate::value!($($v)*);
        $crate::compiler::value::VrlValueConvert::into_expression(value)
    }};
}
