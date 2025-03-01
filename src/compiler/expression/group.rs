use std::fmt;

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    expression::{Executed, Expr, Resolved},
    Context, Expression,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    inner: Box<Expr>,
}

impl Group {
    pub fn new(inner: Expr) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl Expression for Group {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.inner.resolve(ctx)
    }

    fn execute(&self, ctx: &mut Context) -> Executed {
        self.inner.execute(ctx)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        self.inner.type_info(state)
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.inner)
    }
}
