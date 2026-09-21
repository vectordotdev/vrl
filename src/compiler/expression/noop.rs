use std::fmt;

use crate::value::Value;

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{Context, Expression, TypeDef, expression::Resolved};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noop;

impl Expression for Noop {
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(Value::Null)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        TypeInfo::new(state, TypeDef::null())
    }
}

impl fmt::Display for Noop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("null")
    }
}
