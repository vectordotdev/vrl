use crate::path::OwnedTargetPath;
use std::{
    any::{Any, TypeId},
    collections::{BTreeSet, HashMap},
};

type AnyMap = HashMap<TypeId, Box<dyn Any>>;

pub struct CompileConfig {
    /// Custom context injected by the external environment
    custom: AnyMap,
    read_only_paths: BTreeSet<ReadOnlyPath>,
    check_unused_expressions: bool,
}

impl Default for CompileConfig {
    fn default() -> Self {
        CompileConfig {
            custom: AnyMap::default(),
            read_only_paths: BTreeSet::default(),
            check_unused_expressions: true,
        }
    }
}

impl CompileConfig {
    /// Get external context data from the external environment.
    #[must_use]
    pub fn get_custom<T: 'static>(&self) -> Option<&T> {
        self.custom
            .get(&TypeId::of::<T>())
            .and_then(|t| t.downcast_ref())
    }

    /// Get external context data from the external environment.
    pub fn get_custom_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.custom
            .get_mut(&TypeId::of::<T>())
            .and_then(|t| t.downcast_mut())
    }

    /// Sets the external context data for VRL functions to use.
    pub fn set_custom<T: 'static>(&mut self, data: T) {
        self.custom.insert(TypeId::of::<T>(), Box::new(data) as _);
    }

    pub fn custom_mut(&mut self) -> &mut AnyMap {
        &mut self.custom
    }

    /// Marks everything as read only. Any mutations on read-only values will result in a
    /// compile time error.
    pub fn set_read_only(&mut self) {
        self.set_read_only_path(OwnedTargetPath::event_root(), true);
        self.set_read_only_path(OwnedTargetPath::metadata_root(), true);
    }

    #[must_use]
    pub fn is_read_only_path(&self, path: &OwnedTargetPath) -> bool {
        for read_only_path in &self.read_only_paths {
            // any paths that are a parent of read-only paths also can't be modified
            if read_only_path.path.can_start_with(path) {
                return true;
            }

            if read_only_path.recursive {
                if path.can_start_with(&read_only_path.path) {
                    return true;
                }
            } else if path == &read_only_path.path {
                return true;
            }
        }
        false
    }

    /// Adds a path that is considered read only. Assignments to any paths that match
    /// will fail at compile time.
    pub fn set_read_only_path(&mut self, path: OwnedTargetPath, recursive: bool) {
        self.read_only_paths
            .insert(ReadOnlyPath { path, recursive });
    }

    #[must_use]
    pub fn unused_expression_check_enabled(&self) -> bool {
        self.check_unused_expressions
    }

    pub fn disable_unused_expression_check(&mut self) {
        self.check_unused_expressions = false;
    }
}

#[derive(Debug, Clone, Ord, Eq, PartialEq, PartialOrd)]
struct ReadOnlyPath {
    path: OwnedTargetPath,
    recursive: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(PartialEq, Eq, Debug)]
    struct Potato(usize);

    #[test]
    fn can_get_custom() {
        let mut config = CompileConfig::default();
        config.set_custom(Potato(42));

        assert_eq!(&Potato(42), config.get_custom::<Potato>().unwrap());
    }

    #[test]
    fn can_get_custom_mut() {
        let mut config = CompileConfig::default();
        config.set_custom(Potato(42));

        let potato = config.get_custom_mut::<Potato>().unwrap();
        potato.0 = 43;

        assert_eq!(&Potato(43), config.get_custom::<Potato>().unwrap());
    }
}
