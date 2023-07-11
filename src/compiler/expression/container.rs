use std::fmt;

use crate::compiler::{
    expression::{Array, Block, Group, Object, Resolved, Value},
    state::{TypeInfo, TypeState},
    Context, Expression,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    pub variant: Variant,
}

impl Container {
    #[must_use]
    pub fn new(variant: Variant) -> Self {
        Self { variant }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Variant {
    Group(Group),
    Block(Block),
    Array(Array),
    Object(Object),
}

impl Expression for Container {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        use Variant::{Array, Block, Group, Object};

        match &self.variant {
            Group(v) => v.resolve(ctx),
            Block(v) => v.resolve(ctx),
            Array(v) => v.resolve(ctx),
            Object(v) => v.resolve(ctx),
        }
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        use Variant::{Array, Block, Group, Object};

        match &self.variant {
            Group(v) => v.resolve_constant(state),
            Block(v) => v.resolve_constant(state),
            Array(v) => v.resolve_constant(state),
            Object(v) => v.resolve_constant(state),
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        use Variant::{Array, Block, Group, Object};

        match &self.variant {
            Group(v) => v.type_info(state),
            Block(v) => v.type_info(state),
            Array(v) => v.type_info(state),
            Object(v) => v.type_info(state),
        }
    }
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Variant::{Array, Block, Group, Object};

        match &self.variant {
            Group(v) => v.fmt(f),
            Block(v) => v.fmt(f),
            Array(v) => v.fmt(f),
            Object(v) => v.fmt(f),
        }
    }
}

impl From<Group> for Variant {
    fn from(group: Group) -> Self {
        Variant::Group(group)
    }
}

impl From<Block> for Variant {
    fn from(block: Block) -> Self {
        Variant::Block(block)
    }
}

impl From<Array> for Variant {
    fn from(array: Array) -> Self {
        Variant::Array(array)
    }
}

impl From<Object> for Variant {
    fn from(object: Object) -> Self {
        Variant::Object(object)
    }
}
