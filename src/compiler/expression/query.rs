use crate::compiler::{
    Context, Expression,
    expression::{Container, FunctionCall, Resolved, Variable},
    parser::ast::Ident,
    state::ExternalEnv,
    state::{TypeInfo, TypeState},
    type_def::Details,
};
use crate::path::{OwnedTargetPath, OwnedValuePath, PathPrefix};
use crate::value::Value;
use std::fmt;

#[derive(Clone, PartialEq)]
pub struct Query {
    kind: QueryKind,
}

#[derive(Clone, PartialEq)]
enum QueryKind {
    External(OwnedTargetPath),
    Internal {
        variable: Variable,
        path: OwnedValuePath,
    },
    FunctionCall {
        call: FunctionCall,
        path: OwnedValuePath,
    },
    Container {
        container: Container,
        path: OwnedValuePath,
    },
}

impl Query {
    // TODO:
    // - error when trying to index into object
    // - error when trying to path into array
    #[must_use]
    pub fn new(target: Target, path: OwnedValuePath) -> Self {
        let kind = match target {
            Target::External(prefix) => QueryKind::External(OwnedTargetPath { prefix, path }),
            Target::Internal(variable) => QueryKind::Internal { variable, path },
            Target::FunctionCall(call) => QueryKind::FunctionCall { call, path },
            Target::Container(container) => QueryKind::Container { container, path },
        };
        Self { kind }
    }

    #[must_use]
    pub fn path(&self) -> &OwnedValuePath {
        match &self.kind {
            QueryKind::External(path) => &path.path,
            QueryKind::Internal { path, .. }
            | QueryKind::FunctionCall { path, .. }
            | QueryKind::Container { path, .. } => path,
        }
    }

    /// Borrow the query's target without cloning its expression.
    ///
    /// Match on [`TargetRef`] to inspect the target. External prefixes are returned
    /// by value; variables, function calls, and containers remain borrowed.
    #[must_use]
    pub fn target(&self) -> TargetRef<'_> {
        match &self.kind {
            QueryKind::External(path) => TargetRef::External(path.prefix),
            QueryKind::Internal { variable, .. } => TargetRef::Internal(variable),
            QueryKind::FunctionCall { call, .. } => TargetRef::FunctionCall(call),
            QueryKind::Container { container, .. } => TargetRef::Container(container),
        }
    }

    #[must_use]
    pub fn is_external(&self) -> bool {
        matches!(self.kind, QueryKind::External(_))
    }

    #[must_use]
    pub fn external_path(&self) -> Option<&OwnedTargetPath> {
        match &self.kind {
            QueryKind::External(path) => Some(path),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_variable(&self) -> Option<&Variable> {
        match self.target() {
            TargetRef::Internal(variable) => Some(variable),
            _ => None,
        }
    }

    #[must_use]
    pub fn variable_ident(&self) -> Option<&Ident> {
        self.as_variable().map(Variable::ident)
    }

    #[must_use]
    pub fn expression_target(&self) -> Option<&dyn Expression> {
        match self.target() {
            TargetRef::FunctionCall(expr) => Some(expr),
            TargetRef::Container(expr) => Some(expr),
            _ => None,
        }
    }

    // Only "external" paths are supported. Non external paths are ignored
    // see: https://github.com/vectordotdev/vector/issues/11246
    pub fn delete_type_def(&self, external: &mut ExternalEnv, compact: bool) {
        if let Some(target_path) = self.external_path() {
            match target_path.prefix {
                PathPrefix::Event => {
                    let mut type_def = external.target().type_def.clone();
                    type_def.remove(&target_path.path, compact);
                    external.update_target(Details {
                        type_def,
                        value: None,
                    });
                }
                PathPrefix::Metadata => {
                    let mut kind = external.metadata_kind().clone();
                    kind.remove(&target_path.path, compact);
                    external.update_metadata(kind);
                }
            }
        }
    }
}

impl Expression for Query {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        use QueryKind::{Container, External, FunctionCall, Internal};

        let value = match &self.kind {
            External(path) => {
                return Ok(ctx
                    .target()
                    .target_get(path)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or(Value::Null));
            }
            Internal { variable, .. } => variable.resolve(ctx)?,
            FunctionCall { call, .. } => call.resolve(ctx)?,
            Container { container, .. } => container.resolve(ctx)?,
        };

        Ok(value.get(self.path()).cloned().unwrap_or(Value::Null))
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        match self.target() {
            TargetRef::Internal(variable) => variable
                .resolve_constant(state)
                .and_then(|v| v.get(self.path()).cloned()),
            _ => None,
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        use TargetRef::{Container, External, FunctionCall, Internal};

        match self.target() {
            External(prefix) => {
                let result = state.external.kind(prefix).at_path(self.path()).into();
                TypeInfo::new(state, result)
            }
            Internal(variable) => {
                let result = variable.type_def(state).at_path(self.path());
                TypeInfo::new(state, result)
            }
            FunctionCall(call) => call
                .type_info(state)
                .map_result(|result| result.at_path(self.path())),
            Container(container) => container
                .type_info(state)
                .map_result(|result| result.at_path(self.path())),
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = self.target();
        match target {
            TargetRef::Internal(_)
                if !self.path().is_root() && !self.path().segments.first().unwrap().is_index() =>
            {
                write!(f, "{}.{}", target, self.path())
            }
            _ => write!(f, "{}{}", target, self.path()),
        }
    }
}

impl fmt::Debug for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Query({:?}, {:?})", self.target(), self.path())
    }
}

/// An owned target used to construct a [`Query`].
#[derive(Clone, PartialEq)]
pub enum Target {
    Internal(Variable),
    External(PathPrefix),
    FunctionCall(FunctionCall),
    Container(Container),
}

/// A borrowed view of a query target. The external prefix is copied from its stored path.
#[derive(Clone, Copy, PartialEq)]
pub enum TargetRef<'a> {
    Internal(&'a Variable),
    External(PathPrefix),
    FunctionCall(&'a FunctionCall),
    Container(&'a Container),
}

impl<'a> From<&'a Target> for TargetRef<'a> {
    fn from(target: &'a Target) -> Self {
        match target {
            Target::Internal(variable) => Self::Internal(variable),
            Target::External(prefix) => Self::External(*prefix),
            Target::FunctionCall(call) => Self::FunctionCall(call),
            Target::Container(container) => Self::Container(container),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        TargetRef::from(self).fmt(f)
    }
}

impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        TargetRef::from(self).fmt(f)
    }
}

impl fmt::Display for TargetRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TargetRef::{Container, External, FunctionCall, Internal};

        match self {
            Internal(v) => v.fmt(f),
            External(prefix) => match prefix {
                PathPrefix::Event => write!(f, "."),
                PathPrefix::Metadata => write!(f, "%"),
            },
            FunctionCall(v) => v.fmt(f),
            Container(v) => v.fmt(f),
        }
    }
}

impl fmt::Debug for TargetRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TargetRef::{Container, External, FunctionCall, Internal};

        match self {
            Internal(v) => write!(f, "Internal({v:?})"),
            External(prefix) => match prefix {
                PathPrefix::Event => f.write_str("External(Event)"),
                PathPrefix::Metadata => f.write_str("External(Metadata)"),
            },
            FunctionCall(v) => v.fmt(f),
            Container(v) => v.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::expression::Expr;

    #[test]
    fn external_paths_preserve_prefixes_and_formatting() {
        for (prefix, marker, label) in [
            (PathPrefix::Event, ".", "Event"),
            (PathPrefix::Metadata, "%", "Metadata"),
        ] {
            for path in [
                OwnedValuePath::root(),
                OwnedValuePath::single_field("foo").with_index_appended(0),
            ] {
                let query = Query::new(Target::External(prefix), path.clone());
                let external_path = query.external_path().unwrap();

                assert!(query.is_external());
                assert_eq!(query.target(), TargetRef::External(prefix));
                assert_eq!(external_path.prefix, prefix);
                assert_eq!(query.path(), &path);
                assert!(std::ptr::eq(query.path(), &raw const external_path.path));
                assert_eq!(query.to_string(), format!("{marker}{path}"));
                assert_eq!(
                    format!("{query:?}"),
                    format!("Query(External({label}), {path:?})")
                );
                assert!(query.as_variable().is_none());
                assert!(query.variable_ident().is_none());
                assert!(query.expression_target().is_none());
            }
        }
    }

    #[test]
    fn internal_queries_preserve_formatting_and_constant_lookup() {
        for (source, expected, value) in [
            ("foo = 2; foo", "foo", 2),
            (r#"foo = {"bar": 2}; foo.bar"#, "foo.bar", 2),
            ("foo = [2, 3]; foo[1]", "foo[1]", 3),
        ] {
            let program = crate::compiler::compile(source, &[]).unwrap().program;
            let query = match program.expressions.exprs().last().unwrap() {
                Expr::Query(query) => query.clone(),
                Expr::Variable(variable) => {
                    Query::new(Target::Internal(variable.clone()), OwnedValuePath::root())
                }
                _ => panic!("expected a variable or query"),
            };
            let state = program.final_type_info().state;
            let TargetRef::Internal(variable) = query.target() else {
                panic!("expected an internal target");
            };

            assert_eq!(query.to_string(), expected);
            assert!(std::ptr::eq(query.as_variable().unwrap(), variable));
            assert_eq!(query.variable_ident(), Some(variable.ident()));
            assert_eq!(query.resolve_constant(&state), Some(Value::from(value)));
            assert!(query.type_info(&state).result.is_integer());
            assert!(!query.is_external());
            assert!(query.external_path().is_none());
            assert!(query.expression_target().is_none());
        }
    }

    #[cfg(feature = "stdlib-base")]
    #[test]
    fn expression_queries_borrow_their_targets() {
        for (source, is_call) in [
            (r#"{"foo": 5}.foo"#, false),
            (r#"parse_json!(s'{"foo": 5}').foo"#, true),
        ] {
            let program = crate::compiler::compile(source, &crate::stdlib::all())
                .unwrap()
                .program;
            let Expr::Query(query) = program.expressions.exprs().last().unwrap() else {
                panic!("expected a query");
            };

            match query.target() {
                TargetRef::FunctionCall(call) => {
                    assert!(is_call);
                    assert!(std::ptr::addr_eq(
                        query.expression_target().unwrap(),
                        call as &dyn Expression,
                    ));
                }
                TargetRef::Container(container) => {
                    assert!(!is_call);
                    assert!(std::ptr::addr_eq(
                        query.expression_target().unwrap(),
                        container as &dyn Expression,
                    ));
                }
                target => panic!("unexpected target: {target:?}"),
            }
            assert_eq!(query.path(), &OwnedValuePath::single_field("foo"));
            assert!(!query.is_external());
            assert!(query.external_path().is_none());
            assert!(query.as_variable().is_none());
            assert!(query.variable_ident().is_none());
        }
    }

    #[test]
    fn external_root_query_has_infallible_object_type() {
        let query = Query::new(Target::External(PathPrefix::Event), OwnedValuePath::root());

        let state = TypeState::default();
        let type_def = query.type_info(&state).result;

        assert!(type_def.is_infallible());
        assert!(type_def.is_object());

        let object = type_def.as_object().unwrap();

        assert!(object.is_any());
    }
}
