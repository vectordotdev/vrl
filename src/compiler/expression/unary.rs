use std::fmt;

use crate::compiler::{
    Context, Expression,
    expression::{BitwiseNot, Not, Resolved},
    state::{TypeInfo, TypeState},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Unary {
    variant: Variant,
}

impl Unary {
    #[must_use]
    pub fn new(variant: Variant) -> Self {
        Self { variant }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Variant {
    Not(Not),
    BitwiseNot(BitwiseNot),
}

impl Expression for Unary {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        use Variant::{BitwiseNot, Not};

        match &self.variant {
            Not(v) => v.resolve(ctx),
            BitwiseNot(v) => v.resolve(ctx),
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        use Variant::{BitwiseNot, Not};

        let mut state = state.clone();

        let result = match &self.variant {
            Not(v) => v.apply_type_info(&mut state),
            BitwiseNot(v) => v.apply_type_info(&mut state),
        };
        TypeInfo::new(state, result)
    }
}

impl fmt::Display for Unary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Variant::{BitwiseNot, Not};

        match &self.variant {
            Not(v) => v.fmt(f),
            BitwiseNot(v) => v.fmt(f),
        }
    }
}

impl From<Not> for Variant {
    fn from(not: Not) -> Self {
        Variant::Not(not)
    }
}

impl From<BitwiseNot> for Variant {
    fn from(bitwise_not: BitwiseNot) -> Self {
        Variant::BitwiseNot(bitwise_not)
    }
}
