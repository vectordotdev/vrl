//! `TypeDefs`
//!
//! The type definitions for typedefs record the various possible type definitions for the state
//! that can be passed through a VRL program.
//!
//! `TypeDef` contains a `KindInfo`.
//!
//! `KindInfo` can be:
//! `Unknown` - We don't know what type this is.
//! `Known` - A set of the possible known `TypeKind`s. There can be multiple possible types for a
//! path in scenarios such as `if .thing { .x = "hello" } else { .x = 42 }`. In that example after
//! that statement is run, `.x` could contain either an string or an integer, we won't know until
//! runtime exactly which.
//!
//! `TypeKind` is a concrete type for a path, `Bytes` (string), `Integer`, `Float`, `Boolean`,
//! `Timestamp`, `Regex`, `Null` or `Array` or `Object`.
//!
//! `Array` is a Map of `Index` -> `KindInfo`.
//! `Index` can be a specific index into that array, or `Any` which represents any index found within
//! that array.
//!
//! `Object` is a Map of `Field` -> `KindInfo`.
//! `Field` can be a specific field name of the object, or `Any` which represents any element found
//! within that object.

use std::ops::{Deref, DerefMut};

use crate::path::ValuePath;
use crate::value::{
    kind::{merge, Collection, Field, Index},
    Kind, Value,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Fallibility {
    CannotFail,
    MightFail,
    AlwaysFails,
}

impl Fallibility {
    #[must_use]
    /// Merges two [`Fallibility`] values using the following rules:
    ///
    /// - Merging with [`Fallibility::AlwaysFails`] always results in [`Fallibility::AlwaysFails`].
    /// - Merging [`Fallibility::MightFail`] with any variant results in [`Fallibility::MightFail`].
    /// - Merging two [`Fallibility::CannotFail`] values results in [`Fallibility::CannotFail`].
    ///
    /// This is useful for combining the fallibility of sub-expressions.
    pub fn merge(left: &Self, right: &Self) -> Self {
        use Fallibility::{AlwaysFails, CannotFail, MightFail};

        match (left, right) {
            (AlwaysFails, _) | (_, AlwaysFails) => AlwaysFails,
            (MightFail, _) | (_, MightFail) => MightFail,
            _ => CannotFail,
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum Purity {
    /// Used for functions that are idempotent and have no side effects.
    /// The vast majority of VRL expressions (and functions) are pure.
    #[default]
    Pure,
    /// Used for impure functions.
    Impure,
}

impl Purity {
    #[must_use]
    /// Merges two [`Purity`] values. There is only one rule, [`Purity::Impure`] trumps [`Purity::Pure`].
    fn merge(left: &Self, right: &Self) -> Self {
        use Purity::{Impure, Pure};

        match (left, right) {
            (Pure, Pure) => Pure,
            (Impure, _) => Impure,
            (_, Impure) => Impure,
        }
    }
}

/// Properties for a given expression that express the expected outcome of the
/// expression.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TypeDef {
    /// If the function *might* fail, the fallibility must not be [`Fallibility::CannotFail`].
    ///
    /// If the function *might* succeed, the fallibility must not be [`Fallibility::AlwaysFails`].
    ///
    /// Prefer [`Fallibility::AlwaysFails`] over [`Fallibility::MightFail`] whenever possible. If not possible,
    /// choose [`Fallibility::MightFail`].
    ///
    /// Some expressions are infallible e.g. the [`Literal`][crate::expression::Literal] expression, or any
    // custom function designed to be infallible.
    fallibility: Fallibility,

    /// The [`Kind`][value::Kind]s this definition represents.
    kind: Kind,

    /// A function is [`Purity::Pure`] if it is idempotent and has no side effects.
    /// Otherwise, it is [`Purity::Impure`].
    purity: Purity,

    /// The union of [`Kind`][value::Kind]s that can be returned from a nested expression.
    returns: Kind,
}

impl Deref for TypeDef {
    type Target = Kind;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

impl DerefMut for TypeDef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.kind
    }
}

impl TypeDef {
    #[must_use]
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    #[must_use]
    pub fn kind_mut(&mut self) -> &mut Kind {
        &mut self.kind
    }

    #[must_use]
    pub fn returns(&self) -> &Kind {
        &self.returns
    }

    #[must_use]
    pub fn returns_mut(&mut self) -> &mut Kind {
        &mut self.returns
    }

    #[must_use]
    pub fn at_path<'a>(&self, path: impl ValuePath<'a>) -> TypeDef {
        Self {
            fallibility: self.fallibility.clone(),
            kind: self.kind.at_path(path),
            purity: self.purity.clone(),
            returns: self.returns.clone(),
        }
    }

    #[inline]
    #[must_use]
    pub fn fallible(mut self) -> Self {
        self.fallibility = Fallibility::MightFail;
        self
    }

    #[inline]
    #[must_use]
    pub fn infallible(mut self) -> Self {
        self.fallibility = Fallibility::CannotFail;
        self
    }

    #[inline]
    #[must_use]
    pub fn always_fails(mut self) -> Self {
        self.fallibility = Fallibility::AlwaysFails;
        self
    }

    #[inline]
    #[must_use]
    /// Provided for backwards compatibility. Prefer `with_fallibility` for new code.
    pub fn maybe_fallible(mut self, might_fail: bool) -> Self {
        if might_fail {
            self.fallibility = Fallibility::MightFail;
        } else {
            self.fallibility = Fallibility::CannotFail;
        }
        self
    }

    #[inline]
    #[must_use]
    pub fn with_fallibility(mut self, fallibility: Fallibility) -> Self {
        self.fallibility = fallibility;
        self
    }

    #[inline]
    #[must_use]
    pub fn pure(mut self) -> Self {
        self.purity = Purity::Pure;
        self
    }

    #[inline]
    #[must_use]
    pub fn impure(mut self) -> Self {
        self.purity = Purity::Impure;
        self
    }

    #[inline]
    #[must_use]
    pub fn any() -> Self {
        Kind::any().into()
    }

    #[inline]
    #[must_use]
    pub fn bytes() -> Self {
        Kind::bytes().into()
    }

    #[inline]
    #[must_use]
    pub fn or_bytes(mut self) -> Self {
        self.kind.add_bytes();
        self
    }

    #[inline]
    #[must_use]
    pub fn integer() -> Self {
        Kind::integer().into()
    }

    #[inline]
    #[must_use]
    pub fn or_integer(mut self) -> Self {
        self.kind.add_integer();
        self
    }

    #[inline]
    #[must_use]
    pub fn float() -> Self {
        Kind::float().into()
    }

    #[inline]
    #[must_use]
    pub fn or_float(mut self) -> Self {
        self.kind.add_float();
        self
    }

    #[inline]
    #[must_use]
    pub fn boolean() -> Self {
        Kind::boolean().into()
    }

    #[inline]
    #[must_use]
    pub fn or_boolean(mut self) -> Self {
        self.kind.add_boolean();
        self
    }

    #[inline]
    #[must_use]
    pub fn timestamp() -> Self {
        Kind::timestamp().into()
    }

    #[inline]
    #[must_use]
    pub fn or_timestamp(mut self) -> Self {
        self.kind.add_timestamp();
        self
    }

    #[inline]
    #[must_use]
    pub fn regex() -> Self {
        Kind::regex().into()
    }

    #[inline]
    #[must_use]
    pub fn or_regex(mut self) -> Self {
        self.kind.add_regex();
        self
    }

    #[inline]
    #[must_use]
    pub fn null() -> Self {
        Kind::null().into()
    }

    #[inline]
    #[must_use]
    pub fn or_null(mut self) -> Self {
        self.kind.add_null();
        self
    }

    #[inline]
    #[must_use]
    pub fn undefined() -> Self {
        Kind::undefined().into()
    }

    #[inline]
    #[must_use]
    pub fn or_undefined(mut self) -> Self {
        self.kind.add_undefined();
        self
    }

    #[inline]
    #[must_use]
    pub fn never() -> Self {
        Kind::never().into()
    }

    #[inline]
    #[must_use]
    pub fn add_null(mut self) -> Self {
        self.kind.add_null();
        self
    }

    #[inline]
    pub fn array(collection: impl Into<Collection<Index>>) -> Self {
        Kind::array(collection).into()
    }

    #[inline]
    pub fn or_array(mut self, collection: impl Into<Collection<Index>>) -> Self {
        self.kind.add_array(collection);
        self
    }

    /// Convert the [`TypeDef`]s [`Kind`] to an array.
    ///
    /// If `Kind` already has the array state, all other states are removed. If it does not yet
    /// have an array, then equally all existing states are removed, and an "any" array state is
    /// added.
    ///
    /// `TypeDef`s fallibility is kept unmodified.
    #[inline]
    #[must_use]
    pub fn restrict_array(self) -> Self {
        let fallible = self.fallibility;
        let collection = match self.kind.into_array() {
            Some(array) => array,
            None => Collection::any(),
        };

        Self {
            fallibility: fallible,
            kind: Kind::array(collection),
            purity: self.purity.clone(),
            returns: self.returns.clone(),
        }
    }

    #[inline]
    pub fn object(collection: impl Into<Collection<Field>>) -> Self {
        Kind::object(collection).into()
    }

    #[inline]
    pub fn or_object(mut self, collection: impl Into<Collection<Field>>) -> Self {
        self.kind.add_object(collection);
        self
    }

    /// Convert the [`TypeDef`]s [`Kind`] to an object.
    ///
    /// If `Kind` already has the object state, all other states are removed. If it does not yet
    /// have an object, then equally all existing states are removed, and an "any" object state is
    /// added.
    ///
    /// `TypeDef`s fallibility is kept unmodified.
    #[inline]
    #[must_use]
    pub fn restrict_object(self) -> Self {
        let fallible = self.fallibility;
        let collection = match self.kind.into_object() {
            Some(object) => object,
            None => Collection::any(),
        };

        Self {
            fallibility: fallible,
            kind: Kind::object(collection),
            purity: self.purity.clone(),
            returns: self.returns.clone(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }

    #[inline]
    #[must_use]
    pub fn with_returns(mut self, returns: Kind) -> Self {
        self.returns = returns;
        self
    }

    /// VRL has an interesting property where accessing an undefined value "upgrades"
    /// it to a "null" value.
    /// This should be used in places those implicit upgrades can occur.
    // see: https://github.com/vectordotdev/vector/issues/13594
    #[must_use]
    pub fn upgrade_undefined(mut self) -> Self {
        self.kind = self.kind.upgrade_undefined();
        self
    }

    /// Collects any subtypes that can contain multiple indexed types (array, object) and collects
    /// them into a single type for all indexes.
    ///
    /// Used for functions that cant determine which indexes of a collection have been used in the
    /// result.
    #[must_use]
    pub fn collect_subtypes(mut self) -> Self {
        if let Some(object) = self.kind.as_object_mut() {
            object.set_unknown(Kind::undefined());
            object.anonymize();
        }
        if let Some(array) = self.kind.as_array_mut() {
            array.set_unknown(Kind::undefined());
            array.anonymize();
        }

        self
    }

    // -------------------------------------------------------------------------

    #[must_use]
    pub fn is_fallible(&self) -> bool {
        self.fallibility == Fallibility::MightFail || self.fallibility == Fallibility::AlwaysFails
    }

    #[must_use]
    pub fn is_infallible(&self) -> bool {
        !self.is_fallible()
    }

    #[must_use]
    pub fn is_pure(&self) -> bool {
        self.purity == Purity::Pure
    }

    #[must_use]
    pub fn is_impure(&self) -> bool {
        self.purity == Purity::Impure
    }

    /// Set the type definition to be fallible if its kind is not contained
    /// within the provided kind.
    pub fn fallible_unless(mut self, kind: impl Into<Kind>) -> Self {
        let kind = kind.into();
        if kind.is_superset(&self.kind).is_err() {
            self.fallibility = Fallibility::MightFail
        }

        self
    }

    #[must_use]
    pub fn union(mut self, other: Self) -> Self {
        self.fallibility = Fallibility::merge(&self.fallibility, &other.fallibility);
        self.kind = self.kind.union(other.kind);
        self.purity = Purity::merge(&self.purity, &other.purity);
        self.returns = self.returns.union(other.returns);
        self
    }

    // deprecated
    pub fn merge(&mut self, other: Self, strategy: merge::Strategy) {
        self.fallibility = Fallibility::merge(&self.fallibility, &other.fallibility);
        self.kind.merge(other.kind, strategy);
        self.purity = Purity::merge(&self.purity, &other.purity);
        self.returns = self.returns.union(other.returns);
    }

    #[must_use]
    pub fn with_type_inserted<'a>(self, path: impl ValuePath<'a>, other: Self) -> Self {
        let mut kind = self.kind;
        kind.insert(path, other.kind);
        Self {
            fallibility: Fallibility::merge(&self.fallibility, &other.fallibility),
            kind,
            purity: Purity::merge(&self.purity, &other.purity),
            returns: self.returns.clone(),
        }
    }

    #[must_use]
    // deprecated
    pub fn merge_overwrite(mut self, other: Self) -> Self {
        self.merge(
            other,
            merge::Strategy {
                collisions: merge::CollisionStrategy::Overwrite,
            },
        );
        self
    }
}

impl From<Kind> for TypeDef {
    fn from(kind: Kind) -> Self {
        Self {
            fallibility: Fallibility::CannotFail,
            kind,
            purity: Purity::Pure,
            returns: Kind::never(),
        }
    }
}

impl From<TypeDef> for Kind {
    fn from(type_def: TypeDef) -> Self {
        type_def.kind
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Details {
    pub(crate) type_def: TypeDef,
    pub(crate) value: Option<Value>,
}

impl Details {
    /// Returns the union of 2 possible states
    pub(crate) fn merge(self, other: Self) -> Self {
        Self {
            type_def: self.type_def.union(other.type_def),
            value: if self.value == other.value {
                self.value
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::Fallibility::*;
    use super::Purity::*;
    use super::*;

    #[test]
    fn merge_details_same_literal() {
        let a = Details {
            type_def: TypeDef::integer(),
            value: Some(Value::from(5)),
        };
        let b = Details {
            type_def: TypeDef::float(),
            value: Some(Value::from(5)),
        };
        assert_eq!(
            a.merge(b),
            Details {
                type_def: TypeDef::integer().or_float(),
                value: Some(Value::from(5)),
            }
        )
    }

    #[test]
    fn merge_details_different_literal() {
        let a = Details {
            type_def: TypeDef::any(),
            value: Some(Value::from(5)),
        };
        let b = Details {
            type_def: TypeDef::object(Collection::empty()),
            value: Some(Value::from(6)),
        };
        assert_eq!(
            a.merge(b),
            Details {
                type_def: TypeDef::any(),
                value: None,
            }
        )
    }

    #[test]
    fn merge_fallibility_instances() {
        assert_eq!(Fallibility::merge(&AlwaysFails, &MightFail), AlwaysFails);
        assert_eq!(Fallibility::merge(&AlwaysFails, &CannotFail), AlwaysFails);
        assert_eq!(
            Fallibility::merge(&Fallibility::merge(&CannotFail, &MightFail), &AlwaysFails),
            AlwaysFails
        );

        assert_eq!(Fallibility::merge(&MightFail, &MightFail), MightFail);
        assert_eq!(Fallibility::merge(&CannotFail, &MightFail), MightFail);

        assert_eq!(Fallibility::merge(&CannotFail, &CannotFail), CannotFail);
    }

    #[test]
    fn merge_purity() {
        assert_eq!(Purity::merge(&Pure, &Impure), Impure);
        assert_eq!(Purity::merge(&Impure, &Pure), Impure);
        assert_eq!(Purity::merge(&Impure, &Impure), Impure);
        assert_eq!(Purity::merge(&Pure, &Pure), Pure);
    }
}
