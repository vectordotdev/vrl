use crate::path::PathPrefix;
use crate::value::{Kind, Value};
use std::collections::{hash_map::Entry, HashMap};

use super::{parser::ast::Ident, type_def::Details, value::Collection, TypeDef};

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub state: TypeState,
    pub result: TypeDef,
}

impl TypeInfo {
    pub fn new(state: impl Into<TypeState>, result: TypeDef) -> Self {
        Self {
            state: state.into(),
            result,
        }
    }

    pub fn map_result(self, f: impl FnOnce(TypeDef) -> TypeDef) -> Self {
        Self {
            state: self.state,
            result: f(self.result),
        }
    }
}

impl From<&TypeState> for TypeState {
    fn from(state: &TypeState) -> Self {
        state.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TypeState {
    pub local: LocalEnv,
    pub external: ExternalEnv,
}

impl TypeState {
    pub fn merge(self, other: Self) -> Self {
        Self {
            local: self.local.merge(other.local),
            external: self.external.merge(other.external),
        }
    }
}

/// Local environment, limited to a given scope.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LocalEnv {
    pub(crate) bindings: HashMap<Ident, Details>,
}

impl LocalEnv {
    pub(crate) fn variable_idents(&self) -> impl Iterator<Item = &Ident> + '_ {
        self.bindings.keys()
    }

    pub(crate) fn variable(&self, ident: &Ident) -> Option<&Details> {
        self.bindings.get(ident)
    }

    pub(crate) fn insert_variable(&mut self, ident: Ident, details: Details) {
        self.bindings.insert(ident, details);
    }

    pub(crate) fn remove_variable(&mut self, ident: &Ident) -> Option<Details> {
        self.bindings.remove(ident)
    }

    /// Any state the child scope modified that was part of the parent is copied to the parent scope
    pub(crate) fn apply_child_scope(mut self, child: Self) -> Self {
        for (ident, child_details) in child.bindings {
            if let Some(self_details) = self.bindings.get_mut(&ident) {
                *self_details = child_details;
            }
        }

        self
    }

    /// Merges two local envs together. This is useful in cases such as if statements
    /// where different `LocalEnv`'s can be created, and the result is decided at runtime.
    /// The compile-time type must be the union of the options.
    pub(crate) fn merge(mut self, other: Self) -> Self {
        for (ident, other_details) in other.bindings {
            if let Some(self_details) = self.bindings.get_mut(&ident) {
                *self_details = self_details.clone().merge(other_details);
            } else {
                self.bindings.insert(ident, other_details);
            }
        }
        self
    }
}

/// A lexical scope within the program.
#[derive(Debug, Clone)]
pub struct ExternalEnv {
    /// The external target of the program.
    target: Details,

    /// The type of metadata
    metadata: Kind,
}

impl Default for ExternalEnv {
    fn default() -> Self {
        Self::new_with_kind(
            Kind::object(Collection::any()),
            Kind::object(Collection::any()),
        )
    }
}

impl ExternalEnv {
    pub fn merge(self, other: Self) -> Self {
        Self {
            target: self.target.merge(other.target),
            metadata: self.metadata.union(other.metadata),
        }
    }

    /// Creates a new external environment that starts with an initial given
    /// [`Kind`].
    #[must_use]
    pub fn new_with_kind(target: Kind, metadata: Kind) -> Self {
        Self {
            target: Details {
                type_def: target.into(),
                value: None,
            },
            metadata,
        }
    }

    pub(crate) fn target(&self) -> &Details {
        &self.target
    }

    pub fn target_kind(&self) -> &Kind {
        self.target().type_def.kind()
    }

    pub fn kind(&self, prefix: PathPrefix) -> Kind {
        match prefix {
            PathPrefix::Event => self.target_kind(),
            PathPrefix::Metadata => self.metadata_kind(),
        }
        .clone()
    }

    pub fn metadata_kind(&self) -> &Kind {
        &self.metadata
    }

    pub(crate) fn update_target(&mut self, details: Details) {
        self.target = details;
    }

    pub fn update_metadata(&mut self, kind: Kind) {
        self.metadata = kind;
    }
}

/// The state used at runtime to track changes as they happen.
#[derive(Debug, Default)]
pub struct RuntimeState {
    /// The [`Value`] stored in each variable.
    variables: HashMap<Ident, Value>,
}

impl RuntimeState {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn clear(&mut self) {
        self.variables.clear();
    }

    #[must_use]
    pub fn variable(&self, ident: &Ident) -> Option<&Value> {
        self.variables.get(ident)
    }

    pub fn variable_mut(&mut self, ident: &Ident) -> Option<&mut Value> {
        self.variables.get_mut(ident)
    }

    pub(crate) fn insert_variable(&mut self, ident: Ident, value: Value) {
        self.variables.insert(ident, value);
    }

    pub(crate) fn remove_variable(&mut self, ident: &Ident) {
        self.variables.remove(ident);
    }

    pub(crate) fn swap_variable(&mut self, ident: Ident, value: Value) -> Option<Value> {
        match self.variables.entry(ident) {
            Entry::Occupied(mut v) => Some(std::mem::replace(v.get_mut(), value)),
            Entry::Vacant(v) => {
                v.insert(value);
                None
            }
        }
    }
}
