use crate::compiler::{Context, Expression, state};
use crate::core::Value;
use crate::prelude::{ValueError, VrlValueConvert};

#[derive(Clone, Debug)]
pub(crate) enum ResolveBool {
    Constant(bool),
    Expression(Box<dyn Expression>),
}

impl ResolveBool {
    pub fn new(
        expression: Box<dyn Expression>,
        state: &state::TypeState,
    ) -> Result<Self, ValueError> {
        match expression.resolve_constant(state) {
            None => Ok(Self::Expression(expression)),
            Some(constant) => Ok(Self::Constant(constant.try_boolean()?)),
        }
    }

    pub fn new_with_default(
        expression: Option<Box<dyn Expression>>,
        state: &state::TypeState,
        default: &'static Value,
    ) -> Result<Self, ValueError> {
        debug_assert!(
            default.is_boolean(),
            "default value for ResolvedBool must be boolean"
        );
        match expression {
            None => match default {
                Value::Boolean(bool) => Ok(Self::Constant(*bool)),
                _ => unreachable!("Invalid default value type provided, must be boolean"),
            },
            Some(expression) => Self::new(expression, state),
        }
    }

    #[inline]
    pub fn resolve(&self, ctx: &mut Context) -> Result<bool, ValueError> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Expression(expression) => expression.resolve(ctx)?.try_boolean(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ResolveInteger {
    Constant(i64),
    Expression(Box<dyn Expression>),
}

impl ResolveInteger {
    pub fn new(
        expression: Box<dyn Expression>,
        state: &state::TypeState,
    ) -> Result<Self, ValueError> {
        match expression.resolve_constant(state) {
            None => Ok(Self::Expression(expression)),
            Some(constant) => Ok(Self::Constant(constant.try_integer()?)),
        }
    }

    pub fn new_with_default(
        expression: Option<Box<dyn Expression>>,
        state: &state::TypeState,
        default: &'static Value,
    ) -> Result<Self, ValueError> {
        debug_assert!(
            default.is_integer(),
            "default value for ResolveInteger must be integer"
        );
        match expression {
            None => match default {
                Value::Integer(integer) => Ok(ResolveInteger::Constant(*integer)),
                _ => unreachable!("Invalid default value type provided, must be integer"),
            },
            Some(expression) => Self::new(expression, state),
        }
    }

    #[inline]
    pub fn resolve(&self, ctx: &mut Context) -> Result<i64, ValueError> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Expression(expression) => expression.resolve(ctx)?.try_integer(),
        }
    }
}
/*
#[derive(Clone, Debug)]
pub(crate) enum ResolveUsize {
    Constant(usize),
    Expression(Box<dyn Expression>),
}

impl ResolveUsize {
    pub fn new(
        expression: Box<dyn Expression>,
        state: &state::TypeState,
    ) -> Result<Self, ValueError> {
        match expression.resolve_constant(state) {
            None => Ok(Self::Expression(expression)),
            Some(constant) => {
                let integer = constant.try_integer()?;
                match usize::try_from(integer) {
                    Ok(usize) => Ok(Self::Constant(usize)),
                    Err(_) => Err(function::Error::InvalidArgument {
                        keyword: "chunk_size",
                        value: constant,
                        error: r#""chunk_size" is too large"#,
                    }.into())
                }
            },
        }
    }
}*/