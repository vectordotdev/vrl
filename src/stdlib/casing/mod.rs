use convert_case::{Boundary, Case, Casing};

use crate::prelude::*;
use crate::value;

cfg_if::cfg_if! {
    if #[cfg(feature = "stdlib")] {
        pub(crate) mod camelcase;
        pub(crate) mod pascalcase;
        pub(crate) mod snakecase;
        pub(crate) mod screamingsnakecase;
        pub(crate) mod kebabcase;
    }
}

pub(crate) fn variants() -> Vec<Value> {
    vec![
        value!("camelCase"),
        value!("PascalCase"),
        value!("SCREAMING_SNAKE"),
        value!("snake_case"),
        value!("kebab-case"),
    ]
}

pub(crate) fn boundaries() -> Vec<Value> {
    vec![
        value!("lower_upper"), // Splits "camelCase" into "camel" and "Case"
        value!("upper_lower"), // Rarely used, splits "CamelCase" at "Camel" and "Case"
        value!("upper_upper"), // Splits "ABCdef" into "A" and "BCdef"
        value!("acronym"),     // Splits "XMLHttpRequest" into "XML" and "HttpRequest"
        value!("lower_digit"), // Splits "version2Release" into "version" and "2Release"
        value!("upper_digit"), // Splits "Version2Release" into "Version" and "2Release"
        value!("digit_lower"), // Splits "v2release" into "v2" and "release"
        value!("digit_upper"), // Splits "v2Release" into "v2" and "Release"
    ]
}

pub(crate) fn boundaries_msg() -> String {
    boundaries()
        .into_iter()
        .filter_map(|v| Some(v.as_str()?.into_owned()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn variants_msg() -> String {
    variants()
        .into_iter()
        .filter_map(|v| Some(v.as_str()?.into_owned()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn into_case(s: &str) -> Result<Case, Box<dyn DiagnosticMessage>> {
    match s {
        "camelCase" => Ok(Case::Camel),
        "PascalCase" => Ok(Case::Pascal),
        "SREAMING_SNAKE" => Ok(Case::Constant),
        "snake_case" => Ok(Case::Snake),
        "kebab-case" => Ok(Case::Kebab),
        _ => Err(Box::new(ExpressionError::from(format!(
            "case must match one of: {}",
            variants_msg()
        ))) as Box<dyn DiagnosticMessage>),
    }
}

pub(crate) fn into_boundary(s: &str) -> Result<convert_case::Boundary, Box<dyn DiagnosticMessage>> {
    match s {
        "lower_upper" => Ok(convert_case::Boundary::LOWER_UPPER),
        "upper_lower" => Ok(convert_case::Boundary::UPPER_LOWER),
        "acronym" => Ok(convert_case::Boundary::ACRONYM),
        "lower_digit" => Ok(convert_case::Boundary::LOWER_DIGIT),
        "upper_digit" => Ok(convert_case::Boundary::UPPER_DIGIT),
        "digit_lower" => Ok(convert_case::Boundary::DIGIT_LOWER),
        "digit_upper" => Ok(convert_case::Boundary::DIGIT_UPPER),
        _ => Err(Box::new(ExpressionError::from(format!(
            "boundary must match one of: {}",
            boundaries_msg()
        ))) as Box<dyn DiagnosticMessage>),
    }
}

#[inline]
pub(crate) fn convert_case(value: &Value, to_case: Case, from_case: Option<Case>) -> Resolved {
    let string_value = value.try_bytes_utf8_lossy()?;

    match from_case {
        Some(case) => Ok(string_value.from_case(case).to_case(to_case).into()),
        None => Ok(string_value.to_case(to_case).into()),
    }
}

#[inline]
pub(crate) fn convert_case_with_excluded_boundaries(
    string_value: &str,
    to_case: Case,
    from_case: Option<Case>,
    excluded_boundaries: &[Boundary],
) -> Value {
    match from_case {
        Some(case) => string_value
            .from_case(case)
            .without_boundaries(excluded_boundaries)
            .to_case(to_case)
            .into(),
        None => string_value
            .without_boundaries(excluded_boundaries)
            .to_case(to_case)
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_msg() {
        assert_eq!(
            "camelCase, PascalCase, SCREAMING_SNAKE, snake_case, kebab-case",
            variants_msg()
        );
    }
}
