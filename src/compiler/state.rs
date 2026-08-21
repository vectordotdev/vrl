use crate::path::PathPrefix;
use crate::value::{Kind, Value};
use std::collections::{HashMap, hash_map::Entry};
#[cfg(feature = "execution_timeout")]
use std::time::{Duration, Instant};

use super::{TypeDef, parser::ast::Ident, type_def::Details, value::Collection};

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub state: TypeState,
    pub result: TypeDef,
}

impl TypeInfo {
    #[must_use]
    pub fn new(state: impl Into<TypeState>, result: TypeDef) -> Self {
        Self {
            state: state.into(),
            result,
        }
    }

    #[must_use]
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

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Default)]
pub struct TypeState {
    pub local: LocalEnv,
    pub external: ExternalEnv,
}

impl TypeState {
    #[must_use]
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
    #[must_use]
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
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default)]
pub struct RuntimeState {
    /// The [`Value`] stored in each variable.
    variables: HashMap<Ident, Value>,

    /// An optional wall-clock deadline the running program must not exceed.
    /// See [`RuntimeState::set_timeout`].
    #[cfg(feature = "execution_timeout")]
    timeout: Option<ExecutionTimeout>,
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

#[cfg(feature = "execution_timeout")]
impl RuntimeState {
    /// Bounds the total wall-clock time the program is allowed to spend
    /// resolving expressions. Once set, every expression resolution checks
    /// the deadline and panics if it has passed.
    ///
    /// This is a hard safety net against runaway scripts, not a regular
    /// control-flow mechanism: exceeding the timeout panics rather than
    /// returning a `Terminate` error.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = Some(ExecutionTimeout::new(timeout));
    }

    /// Removes any previously configured timeout.
    pub fn clear_timeout(&mut self) {
        self.timeout = None;
    }

    pub(crate) fn check_timeout(&mut self) {
        if let Some(timeout) = self.timeout.as_mut() {
            timeout.check();
        }
    }
}

/// Number of expression evaluations between deadline checks. Consulting the
/// system clock on every single expression resolution would add needless
/// overhead to the interpreter's hottest path, so the deadline is only
/// actually checked once every `CHECK_INTERVAL` calls.
#[cfg(feature = "execution_timeout")]
const CHECK_INTERVAL: u32 = 1024;

#[cfg(feature = "execution_timeout")]
#[derive(Debug, Clone, Copy)]
struct ExecutionTimeout {
    deadline: Instant,
    calls_until_check: u32,
}

#[cfg(feature = "execution_timeout")]
impl ExecutionTimeout {
    fn new(timeout: Duration) -> Self {
        let now = Instant::now();
        Self {
            deadline: now.checked_add(timeout).unwrap_or(now),
            calls_until_check: CHECK_INTERVAL,
        }
    }

    fn check(&mut self) {
        self.calls_until_check -= 1;
        if self.calls_until_check != 0 {
            return;
        }
        self.calls_until_check = CHECK_INTERVAL;

        assert!(
            Instant::now() < self.deadline,
            "VRL program exceeded its execution timeout"
        );
    }
}

#[cfg(all(test, feature = "execution_timeout"))]
mod execution_timeout_tests {
    use super::{CHECK_INTERVAL, RuntimeState};
    use std::time::Duration;

    #[test]
    fn panics_once_deadline_passes() {
        let mut state = RuntimeState::default();
        state.set_timeout(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..CHECK_INTERVAL {
                state.check_timeout();
            }
        }));

        assert!(result.is_err(), "expected check_timeout to panic");
    }

    #[test]
    fn does_not_check_the_clock_before_the_interval_elapses() {
        let mut state = RuntimeState::default();
        state.set_timeout(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));

        for _ in 0..CHECK_INTERVAL - 1 {
            state.check_timeout();
        }
    }

    #[test]
    fn no_timeout_configured_never_panics() {
        let mut state = RuntimeState::default();

        for _ in 0..CHECK_INTERVAL * 4 {
            state.check_timeout();
        }
    }
}
