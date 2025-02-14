use std::fmt::Debug;

use super::{Matcher, Run};
use crate::datadog::search::{Comparison, ComparisonValue, Field};
use crate::path::PathParseError;
use dyn_clone::{clone_trait_object, DynClone};

/// A `Filter` is a generic type that contains methods that are invoked by the `build_filter`
/// function. Each method returns a heap-allocated `Matcher<V>` (typically a closure) containing
/// logic to determine whether the value matches the filter. A filter is intended to be side-effect
/// free and idempotent, and so only receives an immutable reference to self.
pub trait Filter<V: Debug + Send + Sync + Clone + 'static>: DynClone {
    /// Determine whether a field value exists.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the query contains an invalid path.
    fn exists(&self, field: Field) -> Result<Box<dyn Matcher<V>>, PathParseError>;

    /// Determine whether a field value equals `to_match`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the query contains an invalid path.
    fn equals(&self, field: Field, to_match: &str) -> Result<Box<dyn Matcher<V>>, PathParseError>;

    /// Determine whether a value starts with a prefix.
    ///
    /// # Errors
    /// Will return `Err` if the query contains an invalid path.
    fn prefix(&self, field: Field, prefix: &str) -> Result<Box<dyn Matcher<V>>, PathParseError>;

    /// Determine whether a value matches a wildcard.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the query contains an invalid path.
    fn wildcard(&self, field: Field, wildcard: &str)
        -> Result<Box<dyn Matcher<V>>, PathParseError>;

    /// Compare a field value against `comparison_value`, using one of the `comparator` operators.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the query contains an invalid path.
    fn compare(
        &self,
        field: Field,
        comparator: Comparison,
        comparison_value: ComparisonValue,
    ) -> Result<Box<dyn Matcher<V>>, PathParseError>;

    /// Determine whether a field value falls within a range. By default, this will use
    /// `self.compare` on both the lower and upper bound.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the query contains an invalid path.
    fn range(
        &self,
        field: Field,
        lower: ComparisonValue,
        lower_inclusive: bool,
        upper: ComparisonValue,
        upper_inclusive: bool,
    ) -> Result<Box<dyn Matcher<V>>, PathParseError> {
        match (&lower, &upper) {
            // If both bounds are wildcards, just check that the field exists to catch the
            // special case for "tags".
            (ComparisonValue::Unbounded, ComparisonValue::Unbounded) => self.exists(field),
            // Unbounded lower.
            (ComparisonValue::Unbounded, _) => {
                let op = if upper_inclusive {
                    Comparison::Lte
                } else {
                    Comparison::Lt
                };

                self.compare(field, op, upper)
            }
            // Unbounded upper.
            (_, ComparisonValue::Unbounded) => {
                let op = if lower_inclusive {
                    Comparison::Gte
                } else {
                    Comparison::Gt
                };

                self.compare(field, op, lower)
            }
            // Definitive range.
            _ => {
                let lower_op = if lower_inclusive {
                    Comparison::Gte
                } else {
                    Comparison::Gt
                };

                let upper_op = if upper_inclusive {
                    Comparison::Lte
                } else {
                    Comparison::Lt
                };

                let lower_func = self.compare(field.clone(), lower_op, lower)?;
                let upper_func = self.compare(field, upper_op, upper)?;

                Ok(Run::boxed(move |value| {
                    lower_func.run(value) && upper_func.run(value)
                }))
            }
        }
    }
}

clone_trait_object!(<V>Filter<V>);
