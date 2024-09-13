use convert_case::{Case, Casing};

use crate::prelude::*;

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
        crate::value!("camelCase"),
        crate::value!("PascalCase"),
        crate::value!("SCREAMING_SNAKE"),
        crate::value!("snake_case"),
        crate::value!("kebab-case"),
    ]
}

pub(crate) fn variants_msg() -> String {
    variants()
        .into_iter()
        .filter_map(|v| Some(v.as_str()?.into_owned()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn into_case(s: String) -> Result<Case, Box<dyn DiagnosticMessage>> {
    match s.as_ref() {
        "camelCase" => Ok(Case::Camel),
        "PascalCase" => Ok(Case::Pascal),
        "SREAMING_SNAKE" => Ok(Case::ScreamingSnake),
        "snake_case" => Ok(Case::Snake),
        "kebab-case" => Ok(Case::Kebab),
        _ => Err(Box::new(ExpressionError::from(format!(
            "case must match one of: {}",
            variants_msg()
        ))) as Box<dyn DiagnosticMessage>),
    }
}

#[inline]
pub(crate) fn convert_case(value: Value, to_case: Case, from_case: Option<Case>) -> Resolved {
    match from_case {
        Some(case) => Ok(value
            .try_bytes_utf8_lossy()?
            .from_case(case)
            .to_case(to_case)
            .into()),
        None => Ok(value.try_bytes_utf8_lossy()?.to_case(to_case).into()),
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
