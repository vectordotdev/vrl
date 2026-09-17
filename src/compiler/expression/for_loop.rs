use std::fmt;

use crate::compiler::expression::{Block, Resolved};
use crate::compiler::parser::ast::{ForPattern, Ident};
use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{Context, Expression, ExpressionError, Span, TypeDef, codes};
use crate::diagnostic::{DiagnosticMessage, Label};
use crate::value::kind::Collection;
use crate::value::{Kind, ObjectMap, Value};

#[derive(Debug)]
pub struct For {
    pub span: Span,
    pub pattern: ForPattern,
    pub iterable: Box<dyn Expression>,
    pub block: Block,
}

#[derive(Debug)]
enum LoopItem {
    Single(Value),
    KeyValue(Value, Value),
}

impl For {
    #[must_use]
    pub fn new(
        span: Span,
        pattern: ForPattern,
        iterable: Box<dyn Expression>,
        block: Block,
    ) -> Self {
        Self {
            span,
            pattern,
            iterable,
            block,
        }
    }

    fn resolve_items<'a, I>(
        &self,
        ctx: &mut Context,
        first_ident: Option<&'a Ident>,
        second_ident: Option<&'a Ident>,
        items: I,
    ) -> Resolved
    where
        I: IntoIterator<Item = Result<LoopItem, ExpressionError>>,
    {
        let mut guard = ForScopeGuard::new(ctx, first_ident, second_ident);

        for item_res in items {
            guard.ctx_mut().checkpoint()?;
            let item = item_res?;
            match item {
                LoopItem::Single(value) => guard.set_single(value),
                LoopItem::KeyValue(key, value) => guard.set_key_value(key, value),
            }

            match self.block.resolve(guard.ctx_mut()) {
                Ok(_) | Err(ExpressionError::Continue { .. }) => {}
                Err(ExpressionError::Break { .. }) => break,
                Err(err) => return Err(err),
            }
        }

        Ok(Value::Null)
    }

    fn resolve_array(&self, arr: Vec<Value>, ctx: &mut Context) -> Resolved {
        match &self.pattern {
            ForPattern::Single(val_node) => {
                let first_ident = (!is_wildcard(val_node.inner())).then(|| val_node.inner());
                let items = arr.into_iter().map(|v| Ok(LoopItem::Single(v)));
                self.resolve_items(ctx, first_ident, None, items)
            }
            ForPattern::KeyValue(key_node, val_node) => {
                let has_first = !is_wildcard(key_node.inner());
                let first_ident = has_first.then(|| key_node.inner());
                let second_ident = (!is_wildcard(val_node.inner())).then(|| val_node.inner());

                let span = self.span;
                let items = arr.into_iter().enumerate().map(move |(idx, item)| {
                    let key_val = if has_first {
                        let idx_i64 = i64::try_from(idx).map_err(|_| ExpressionError::Error {
                            message: "array index overflow".to_string(),
                            labels: vec![Label::primary("array index overflow", span)],
                            notes: vec![],
                        })?;
                        Value::Integer(idx_i64)
                    } else {
                        Value::Null
                    };
                    Ok(LoopItem::KeyValue(key_val, item))
                });
                self.resolve_items(ctx, first_ident, second_ident, items)
            }
        }
    }

    fn resolve_object(&self, map: ObjectMap, ctx: &mut Context) -> Resolved {
        match &self.pattern {
            ForPattern::KeyValue(key_node, val_node) => {
                let has_first = !is_wildcard(key_node.inner());
                let first_ident = has_first.then(|| key_node.inner());
                let second_ident = (!is_wildcard(val_node.inner())).then(|| val_node.inner());

                let items = map.into_iter().map(move |(key, val)| {
                    let key_val = if has_first {
                        Value::from(key)
                    } else {
                        Value::Null
                    };
                    Ok(LoopItem::KeyValue(key_val, val))
                });
                self.resolve_items(ctx, first_ident, second_ident, items)
            }
            ForPattern::Single(_) => {
                let message = "iterating over an object requires a key-value pattern: 'for key, value in ...'".to_string();
                Err(ExpressionError::Error {
                    labels: vec![Label::primary(&message, self.span)],
                    message,
                    notes: vec![],
                })
            }
        }
    }
}

impl Clone for For {
    fn clone(&self) -> Self {
        Self {
            span: self.span,
            pattern: self.pattern.clone(),
            iterable: dyn_clone::clone_box(&*self.iterable),
            block: self.block.clone(),
        }
    }
}

impl PartialEq for For {
    fn eq(&self, other: &Self) -> bool {
        self.span == other.span && self.pattern == other.pattern && self.block == other.block
    }
}

impl Expression for For {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let collection = self.iterable.resolve(ctx)?;
        match collection {
            Value::Array(arr) => self.resolve_array(arr, ctx),
            Value::Object(map) => self.resolve_object(map, ctx),
            other => {
                let message = format!("expected array or object, got {}", other.kind_str());
                Err(ExpressionError::Error {
                    labels: vec![Label::primary(&message, self.span)],
                    message,
                    notes: vec![],
                })
            }
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let iterable_info = self.iterable.type_info(state);
        let pre_loop_state = iterable_info.state;
        let mut body_state = pre_loop_state.clone();

        let iterable_kind = iterable_info.result.kind();
        let pattern_idents = bind_for_pattern(&self.pattern, iterable_kind, &mut body_state);

        let body_info = self.block.type_info(&body_state);
        let mut final_body_state = body_info.state;
        restore_pattern_variables(&mut final_body_state, &pre_loop_state, &pattern_idents);

        let final_state = final_body_state.merge(pre_loop_state);
        let fallible = iterable_info.result.is_fallible() || body_info.result.is_fallible();
        TypeInfo::new(
            final_state,
            TypeDef::null()
                .maybe_fallible(fallible)
                .with_returns(body_info.result.returns().clone()),
        )
    }
}

impl fmt::Display for For {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(formatted) = self.iterable.format() {
            write!(f, "for {} in {} {}", self.pattern, formatted, self.block)
        } else {
            write!(
                f,
                "for {} in {:?} {}",
                self.pattern, self.iterable, self.block
            )
        }
    }
}

pub(crate) fn is_wildcard(ident: &Ident) -> bool {
    ident.is_empty() || ident.as_ref() == "_"
}

#[must_use]
pub(crate) fn contains_non_collection_kind(kind: &Kind) -> bool {
    kind.contains_bytes()
        || kind.contains_integer()
        || kind.contains_float()
        || kind.contains_boolean()
        || kind.contains_timestamp()
        || kind.contains_regex()
        || kind.contains_null()
        || kind.contains_undefined()
}

pub(crate) fn bind_for_pattern(
    pattern: &ForPattern,
    iterable_kind: &Kind,
    state: &mut TypeState,
) -> Vec<Ident> {
    let mut pattern_idents = Vec::new();
    match pattern {
        ForPattern::Single(val_node) => {
            let val_ident = val_node.inner();
            if !is_wildcard(val_ident) {
                pattern_idents.push(val_ident.clone());
                let val_kind = iterable_kind
                    .as_array()
                    .map_or_else(Kind::any, Collection::reduced_kind);
                state.local.insert_variable(
                    val_ident.clone(),
                    crate::compiler::type_def::Details {
                        type_def: TypeDef::from(val_kind),
                        value: None,
                    },
                );
            }
        }
        ForPattern::KeyValue(key_node, val_node) => {
            let key_ident = key_node.inner();
            let val_ident = val_node.inner();

            let (key_kind, val_kind) = {
                let has_array = iterable_kind.contains_array();
                let has_object = iterable_kind.contains_object();
                let arr_val = iterable_kind
                    .as_array()
                    .map_or_else(Kind::any, Collection::reduced_kind);
                let obj_val = iterable_kind
                    .as_object()
                    .map_or_else(Kind::any, Collection::reduced_kind);

                if has_array && has_object {
                    (Kind::integer().or_bytes(), arr_val.union(obj_val))
                } else if has_object {
                    (Kind::bytes(), obj_val)
                } else {
                    (Kind::integer(), arr_val)
                }
            };

            if !is_wildcard(key_ident) {
                pattern_idents.push(key_ident.clone());
                state.local.insert_variable(
                    key_ident.clone(),
                    crate::compiler::type_def::Details {
                        type_def: TypeDef::from(key_kind),
                        value: None,
                    },
                );
            }
            if !is_wildcard(val_ident) {
                pattern_idents.push(val_ident.clone());
                state.local.insert_variable(
                    val_ident.clone(),
                    crate::compiler::type_def::Details {
                        type_def: TypeDef::from(val_kind),
                        value: None,
                    },
                );
            }
        }
    }
    pattern_idents
}

pub(crate) fn restore_pattern_variables(
    body_state: &mut TypeState,
    pre_loop_state: &TypeState,
    pattern_idents: &[Ident],
) {
    for ident in pattern_idents {
        match pre_loop_state.local.variable(ident) {
            Some(details) => body_state
                .local
                .insert_variable(ident.clone(), details.clone()),
            None => {
                body_state.local.remove_variable(ident);
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Error {
    variant: ErrorVariant,
    span: Span,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ErrorVariant {
    #[error("expected array or object, got {0}")]
    NonCollection(Kind),
    #[error("iterating over an object requires a key-value pattern: 'for key, value in ...'")]
    SingleVarObject,
    #[error("duplicate variable '{0}' in loop pattern")]
    DuplicatePatternIdent(String),
}

impl Error {
    #[must_use]
    pub(crate) const fn non_collection(span: Span, kind: Kind) -> Self {
        Self {
            variant: ErrorVariant::NonCollection(kind),
            span,
        }
    }

    #[must_use]
    pub(crate) const fn single_var_object(span: Span) -> Self {
        Self {
            variant: ErrorVariant::SingleVarObject,
            span,
        }
    }

    #[must_use]
    pub(crate) fn duplicate_pattern_ident(span: Span, name: String) -> Self {
        Self {
            variant: ErrorVariant::DuplicatePatternIdent(name),
            span,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.variant)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.variant)
    }
}

impl DiagnosticMessage for Error {
    fn code(&self) -> usize {
        match self.variant {
            ErrorVariant::NonCollection(_) => codes::CompilerCode::NonCollectionIterable as usize,
            ErrorVariant::SingleVarObject => codes::CompilerCode::ObjectSingleVarPattern as usize,
            ErrorVariant::DuplicatePatternIdent(_) => {
                codes::CompilerCode::DuplicatePatternIdent as usize
            }
        }
    }

    fn labels(&self) -> Vec<Label> {
        match &self.variant {
            ErrorVariant::NonCollection(_) => {
                vec![Label::primary("expected array or object", self.span)]
            }
            ErrorVariant::SingleVarObject => vec![Label::primary(
                "iterating over an object requires key-value pattern",
                self.span,
            )],
            ErrorVariant::DuplicatePatternIdent(name) => vec![Label::primary(
                format!("duplicate variable '{name}' in loop pattern"),
                self.span,
            )],
        }
    }
}

/// An RAII guard that manages variable bindings across loop iterations.
///
/// Pre-existing outer variables matching the loop variable names have their
/// values saved before the loop begins and restored when the loop exits
/// (via normal completion, break, return, or error).
/// Loop variable slots are updated in-place during the loop without allocations.
/// Loop variables that did not exist before the loop are dropped on exit.
pub struct ForScopeGuard<'a, 'c, 'b> {
    ctx: &'c mut Context<'b>,
    first_ident: Option<&'a Ident>,
    first_old_value: Option<Value>,
    second_ident: Option<&'a Ident>,
    second_old_value: Option<Value>,
}

impl<'a, 'c, 'b> ForScopeGuard<'a, 'c, 'b> {
    #[must_use]
    pub fn new(
        ctx: &'c mut Context<'b>,
        first_ident: Option<&'a Ident>,
        mut second_ident: Option<&'a Ident>,
    ) -> Self {
        let first_old_value = first_ident.and_then(|ident| ctx.state_mut().remove_variable(ident));

        if first_ident == second_ident {
            second_ident = None;
        }

        let second_old_value =
            second_ident.and_then(|ident| ctx.state_mut().remove_variable(ident));

        Self {
            ctx,
            first_ident,
            first_old_value,
            second_ident,
            second_old_value,
        }
    }

    #[inline]
    pub fn ctx_mut(&mut self) -> &mut Context<'b> {
        self.ctx
    }

    #[inline]
    #[must_use]
    pub fn has_first(&self) -> bool {
        self.first_ident.is_some()
    }

    #[inline]
    #[must_use]
    pub fn has_second(&self) -> bool {
        self.second_ident.is_some()
    }

    #[inline]
    pub fn set_first(&mut self, value: Value) {
        if let Some(ident) = self.first_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }
    }

    #[inline]
    pub fn set_second(&mut self, value: Value) {
        if let Some(ident) = self.second_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }
    }

    #[inline]
    pub fn set_single(&mut self, value: Value) {
        self.set_first(value);
    }

    #[inline]
    pub fn set_key_value(&mut self, key: Value, value: Value) {
        self.set_first(key);
        self.set_second(value);
    }
}

impl Drop for ForScopeGuard<'_, '_, '_> {
    fn drop(&mut self) {
        if let Some(ident) = self.first_ident {
            match self.first_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
        if let Some(ident) = self.second_ident {
            match self.second_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::expression::{Expr, Literal};
    use crate::compiler::parser::ast::Node;
    use crate::compiler::state::RuntimeState;
    use crate::compiler::{TargetValue, TimeZone};
    use crate::diagnostic::Span;

    fn test_ctx<'a>(
        target: &'a mut Value,
        state: &'a mut RuntimeState,
        tz: &'a TimeZone,
    ) -> Context<'a> {
        Context::new(target, state, tz)
    }

    #[test]
    fn test_scope_guard_in_place_mutation() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = test_ctx(&mut target, &mut state, &tz);

        let x_id = Ident::new("x");
        let y_id = Ident::new("y");
        let mut guard = ForScopeGuard::new(&mut ctx, Some(&x_id), Some(&y_id));

        guard.set_key_value(Value::Integer(0), Value::from("first"));
        assert_eq!(
            guard.ctx_mut().state().variable(&x_id),
            Some(&Value::Integer(0))
        );
        assert_eq!(
            guard.ctx_mut().state().variable(&y_id),
            Some(&Value::from("first"))
        );

        guard.set_key_value(Value::Integer(1), Value::from("second"));
        assert_eq!(
            guard.ctx_mut().state().variable(&x_id),
            Some(&Value::Integer(1))
        );
        assert_eq!(
            guard.ctx_mut().state().variable(&y_id),
            Some(&Value::from("second"))
        );
    }

    #[test]
    fn test_scope_guard_restores_shadowed_on_drop() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let x_id = Ident::new("x");
        state.insert_variable(x_id.clone(), Value::from("original_x"));

        {
            let mut ctx = test_ctx(&mut target, &mut state, &tz);
            let mut guard = ForScopeGuard::new(&mut ctx, Some(&x_id), None);
            guard.set_single(Value::from("in_loop"));
            assert_eq!(
                guard.ctx_mut().state().variable(&x_id),
                Some(&Value::from("in_loop"))
            );
        }

        assert_eq!(state.variable(&x_id), Some(&Value::from("original_x")));
    }

    #[test]
    fn test_scope_guard_removes_unshadowed_on_drop() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let x_id = Ident::new("x");

        {
            let mut ctx = test_ctx(&mut target, &mut state, &tz);
            let mut guard = ForScopeGuard::new(&mut ctx, Some(&x_id), None);
            guard.set_single(Value::from("in_loop"));
            assert_eq!(
                guard.ctx_mut().state().variable(&x_id),
                Some(&Value::from("in_loop"))
            );
        }

        assert!(state.variable(&x_id).is_none());
    }

    #[test]
    fn test_for_iteration_values_array_single() {
        let span = Span::new(0, 0);
        let x_id = Ident::new("x");
        let pattern = ForPattern::Single(Node::new(span, x_id));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(10),
            Value::from(20),
            Value::from(30),
        ])));

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = test_ctx(&mut target, &mut state, &tz);

        let for_expr = For::new(
            span,
            pattern,
            iterable,
            Block::new_inline(vec![Expr::from(Literal::from(1))]),
        );
        assert_eq!(for_expr.resolve(&mut ctx), Ok(Value::Null));
    }

    #[test]
    fn test_for_iteration_values_array_key_value() {
        let span = Span::new(0, 0);
        let i_id = Ident::new("i");
        let v_id = Ident::new("v");
        let pattern = ForPattern::KeyValue(Node::new(span, i_id), Node::new(span, v_id));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from("alpha"),
            Value::from("beta"),
        ])));

        let for_expr = For::new(
            span,
            pattern,
            iterable,
            Block::new_inline(vec![Expr::from(Literal::from(true))]),
        );

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = test_ctx(&mut target, &mut state, &tz);

        assert_eq!(for_expr.resolve(&mut ctx), Ok(Value::Null));
    }

    #[test]
    fn test_for_iteration_values_object_key_value() {
        let span = Span::new(0, 0);
        let k_id = Ident::new("k");
        let v_id = Ident::new("v");
        let pattern = ForPattern::KeyValue(Node::new(span, k_id), Node::new(span, v_id));

        let mut map = ObjectMap::new();
        map.insert("key1".into(), Value::Integer(100));
        map.insert("key2".into(), Value::Integer(200));
        let iterable = Box::new(Expr::from(Value::Object(map)));

        let for_expr = For::new(
            span,
            pattern,
            iterable,
            Block::new_inline(vec![Expr::from(Literal::from("ok"))]),
        );

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = test_ctx(&mut target, &mut state, &tz);

        assert_eq!(for_expr.resolve(&mut ctx), Ok(Value::Null));
    }

    #[test]
    fn test_for_nested_loops() {
        let span = Span::new(0, 0);
        let inner_for = For::new(
            span,
            ForPattern::Single(Node::new(span, Ident::new("y"))),
            Box::new(Expr::from(Value::Array(vec![
                Value::from(10),
                Value::from(20),
            ]))),
            Block::new_inline(vec![Expr::from(Literal::from(1))]),
        );

        let outer_for = For::new(
            span,
            ForPattern::Single(Node::new(span, Ident::new("x"))),
            Box::new(Expr::from(Value::Array(vec![
                Value::from(1),
                Value::from(2),
            ]))),
            Block::new_inline(vec![Expr::For(inner_for)]),
        );

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = test_ctx(&mut target, &mut state, &tz);

        assert_eq!(outer_for.resolve(&mut ctx), Ok(Value::Null));
        assert!(state.variable(&Ident::new("x")).is_none());
        assert!(state.variable(&Ident::new("y")).is_none());
    }

    #[test]
    fn test_for_display_and_debug() {
        let span = Span::new(0, 0);
        let for_expr = For::new(
            span,
            ForPattern::Single(Node::new(span, Ident::new("x"))),
            Box::new(Expr::from(Value::Array(vec![]))),
            Block::new_inline(vec![]),
        );

        let debug_str = format!("{for_expr:?}");
        assert!(debug_str.contains("For"));

        let display_str = format!("{for_expr}");
        assert!(display_str.starts_with("for x in"));
    }

    #[test]
    fn test_for_type_info() {
        let span = Span::new(0, 0);
        let for_expr = For::new(
            span,
            ForPattern::Single(Node::new(span, Ident::new("x"))),
            Box::new(Expr::from(Value::Array(vec![]))),
            Block::new_inline(vec![Expr::from(Literal::from(42))]),
        );

        let state = TypeState::default();
        let info = for_expr.type_info(&state);
        assert!(info.result.is_null());
    }

    #[test]
    fn test_for_type_info_pattern_variable_binding() {
        // 1. Single pattern: array of integers
        let compilation = crate::compiler::compile("y = 0\nfor x in [1, 2] {\n  y = x\n}", &[])
            .expect("compiles");
        let for_expr = match &compilation.program.expressions.exprs()[1] {
            Expr::For(f) => f,
            other => panic!("expected for expression, got {other:?}"),
        };

        let mut state = TypeState::default();
        state.local.insert_variable(
            Ident::new("y"),
            crate::compiler::type_def::Details {
                type_def: TypeDef::integer(),
                value: None,
            },
        );

        let info = for_expr.type_info(&state);
        assert!(info.result.is_null());
        // Pattern variable must not leak into final state
        assert!(info.state.local.variable(&Ident::new("x")).is_none());
        // Body variable y must have integer kind
        assert_eq!(
            info.state
                .local
                .variable(&Ident::new("y"))
                .expect("y in state")
                .type_def
                .kind(),
            &Kind::integer()
        );

        // 2. KeyValue pattern: object with boolean values
        let compilation_kv = crate::compiler::compile(
            "v_out = false\nfor k, v in { \"a\": true, \"b\": false } {\n  v_out = v\n}",
            &[],
        )
        .expect("compiles");
        let for_kv = match &compilation_kv.program.expressions.exprs()[1] {
            Expr::For(f) => f,
            other => panic!("expected for expression, got {other:?}"),
        };

        let mut state_kv = TypeState::default();
        state_kv.local.insert_variable(
            Ident::new("v_out"),
            crate::compiler::type_def::Details {
                type_def: TypeDef::boolean(),
                value: None,
            },
        );

        let info_kv = for_kv.type_info(&state_kv);
        assert!(info_kv.result.is_null());
        assert!(info_kv.state.local.variable(&Ident::new("k")).is_none());
        assert!(info_kv.state.local.variable(&Ident::new("v")).is_none());
        assert_eq!(
            info_kv
                .state
                .local
                .variable(&Ident::new("v_out"))
                .expect("v_out in state")
                .type_def
                .kind(),
            &Kind::boolean()
        );

        // 3. Shadowing pre-existing variable
        let mut state_shadow = state.clone();
        state_shadow.local.insert_variable(
            Ident::new("x"),
            crate::compiler::type_def::Details {
                type_def: TypeDef::bytes(),
                value: None,
            },
        );
        let info_shadow = for_expr.type_info(&state_shadow);
        assert_eq!(
            info_shadow
                .state
                .local
                .variable(&Ident::new("x"))
                .expect("x restored")
                .type_def
                .kind(),
            &Kind::bytes()
        );
    }

    #[test]
    fn test_for_duplicate_pattern_ident_diagnostic() {
        let Err(err) = crate::compiler::compile("for x, x in [1, 2] { null }", &[]) else {
            panic!("duplicate pattern ident should fail");
        };
        assert_eq!(
            err[0].code,
            codes::CompilerCode::DuplicatePatternIdent as usize
        );
        assert_eq!(err[0].message, "duplicate variable 'x' in loop pattern");

        // Wildcards are allowed to be duplicated
        assert!(crate::compiler::compile("for _, _ in [1, 2] { null }", &[]).is_ok());
    }

    #[test]
    fn test_break_and_continue_diagnostics_code_and_labels() {
        let span = Span::new(42, 47);
        let break_err = ExpressionError::Break { span };
        let continue_err = ExpressionError::Continue { span };

        assert_eq!(
            break_err.code(),
            codes::CompilerCode::BreakOutsideLoop as usize
        );
        assert_eq!(
            continue_err.code(),
            codes::CompilerCode::BreakOutsideLoop as usize
        );
        assert_eq!(
            break_err.labels(),
            vec![Label::primary("break outside of loop", span)]
        );
        assert_eq!(
            continue_err.labels(),
            vec![Label::primary("continue outside of loop", span)]
        );
    }

    use crate::compiler::runtime::Runtime;

    #[test]
    fn test_scope_guard_has_methods() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let k = Ident::new("k");
        let guard = ForScopeGuard::new(&mut ctx, Some(&k), None);
        assert!(guard.has_first());
        assert!(!guard.has_second());
    }

    #[test]
    fn test_scope_guard_selective_setters() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let k_id = Ident::new("k");
        let v_id = Ident::new("v");

        // 1. Both present
        {
            let mut guard = ForScopeGuard::new(&mut ctx, Some(&k_id), Some(&v_id));
            assert!(guard.has_first());
            assert!(guard.has_second());
            guard.set_first(Value::from("key1"));
            guard.set_second(Value::Integer(100));
            assert_eq!(
                guard.ctx_mut().state().variable(&k_id),
                Some(&Value::from("key1"))
            );
            assert_eq!(
                guard.ctx_mut().state().variable(&v_id),
                Some(&Value::Integer(100))
            );
        }

        // 2. Only first present (second wildcard)
        {
            let mut guard_first = ForScopeGuard::new(&mut ctx, Some(&k_id), None);
            assert!(guard_first.has_first());
            assert!(!guard_first.has_second());
            guard_first.set_first(Value::from("key2"));
            guard_first.set_second(Value::Integer(200)); // no-op for second
            assert_eq!(
                guard_first.ctx_mut().state().variable(&k_id),
                Some(&Value::from("key2"))
            );
        }

        // 3. Only second present (first wildcard)
        {
            let mut guard_second = ForScopeGuard::new(&mut ctx, None, Some(&v_id));
            assert!(!guard_second.has_first());
            assert!(guard_second.has_second());
            guard_second.set_first(Value::from("key3")); // no-op for first
            guard_second.set_second(Value::Integer(300));
            assert_eq!(
                guard_second.ctx_mut().state().variable(&v_id),
                Some(&Value::Integer(300))
            );
        }

        // 4. Neither present (both wildcards)
        {
            let mut guard_none = ForScopeGuard::new(&mut ctx, None, None);
            assert!(!guard_none.has_first());
            assert!(!guard_none.has_second());
            guard_none.set_first(Value::from("key4")); // no-op
            guard_none.set_second(Value::Integer(400)); // no-op
        }
    }

    #[test]
    fn test_for_wildcard_iterations() {
        let run = |script: &str| -> Value {
            let mut target = Value::Object(ObjectMap::new());
            let mut runtime = Runtime::new(RuntimeState::default());
            let prog = crate::compiler::compile(script, &[])
                .expect("compiles")
                .program;
            runtime
                .resolve(&mut target, &prog, &TimeZone::default())
                .expect("resolves")
        };

        // 1. Array single wildcard: for _ in [1, 2, 3]
        let res = run("count = 0\nfor _ in [1, 2, 3] {\n  count = count + 1\n}\ncount");
        assert_eq!(res, Value::Integer(3));

        // 2. Array key-value: for _, v in [10, 20]
        let res = run("sum = 0\nfor _, v in [10, 20] {\n  sum = sum + v\n}\nsum");
        assert_eq!(res, Value::Integer(30));

        // 3. Array key-value: for i, _ in [10, 20]
        let res = run("sum_idx = 0\nfor i, _ in [10, 20] {\n  sum_idx = sum_idx + i\n}\nsum_idx");
        assert_eq!(res, Value::Integer(1));

        // 4. Array key-value: for _, _ in [10, 20]
        let res = run("c = 0\nfor _, _ in [10, 20] {\n  c = c + 1\n}\nc");
        assert_eq!(res, Value::Integer(2));

        // 5. Object key-value: for _, v in {"a": 1, "b": 2}
        let res = run(
            "obj_sum = 0\nfor _, v in { \"a\": 1, \"b\": 2 } {\n  obj_sum = obj_sum + v\n}\nobj_sum",
        );
        assert_eq!(res, Value::Integer(3));

        // 6. Object key-value: for k, _ in {"a": 1, "b": 2}
        let res = run(
            "count_k = 0\nfor k, _ in { \"a\": 1, \"b\": 2 } {\n  count_k = count_k + 1\n}\ncount_k",
        );
        assert_eq!(res, Value::Integer(2));

        // 7. Object key-value: for _, _ in {"a": 1, "b": 2}
        let res =
            run("obj_c = 0\nfor _, _ in { \"a\": 1, \"b\": 2 } {\n  obj_c = obj_c + 1\n}\nobj_c");
        assert_eq!(res, Value::Integer(2));
    }

    #[test]
    fn test_array_index_overflow_error_format() {
        let err = ExpressionError::Error {
            message: "array index overflow".to_string(),
            labels: vec![Label::primary("array index overflow", Span::default())],
            notes: vec![],
        };
        assert_eq!(err.message(), "array index overflow");
        assert_eq!(
            err.labels(),
            vec![Label::primary("array index overflow", Span::default())]
        );
    }

    #[test]
    fn test_for_type_info_propagates_fallibility() {
        #[derive(Debug, Clone, PartialEq)]
        struct FallibleMock;
        impl Expression for FallibleMock {
            fn resolve(&self, _ctx: &mut Context) -> Resolved {
                Ok(Value::Array(vec![]))
            }
            fn type_info(&self, state: &TypeState) -> TypeInfo {
                TypeInfo::new(state.clone(), TypeDef::null().fallible())
            }
        }

        let span = Span::new(10, 20);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(FallibleMock);
        let block = Block::new_inline(vec![Expr::from(Literal::from(1))]);
        let for_expr = For::new(span, pattern, iterable, block);

        let state = TypeState::default();
        let info = for_expr.type_info(&state);
        assert!(info.result.is_fallible());
    }

    #[test]
    fn test_for_type_info_propagates_body_fallibility() {
        let Err(err) = crate::compiler::compile("for x in [1, 2] {\n  10 / x\n}", &[]) else {
            panic!("unhandled fallible expression should fail");
        };
        assert_eq!(err[0].code, codes::ExprCode::FallibleExpression as usize);
    }

    #[test]
    fn test_for_runtime_error_preserves_span() {
        let span = Span::new(5, 15);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::from(123)));
        let block = Block::new_inline(vec![]);
        let for_expr = For::new(span, pattern, iterable, block);

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let res = for_expr.resolve(&mut ctx);
        match res {
            Err(ExpressionError::Error { labels, .. }) => {
                assert_eq!(labels.len(), 1);
                assert_eq!(labels[0].span, span);
            }
            other => panic!("expected ExpressionError::Error with span, got {other:?}"),
        }
    }

    #[test]
    fn test_for_runtime_error_object_single_pattern_preserves_span() {
        let span = Span::new(7, 18);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let mut map = ObjectMap::new();
        map.insert("key".into(), Value::from("val"));
        let iterable = Box::new(Expr::from(Value::Object(map)));
        let block = Block::new_inline(vec![]);
        let for_expr = For::new(span, pattern, iterable, block);

        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let res = for_expr.resolve(&mut ctx);
        match res {
            Err(ExpressionError::Error { labels, .. }) => {
                assert_eq!(labels.len(), 1);
                assert_eq!(labels[0].span, span);
            }
            other => panic!("expected ExpressionError::Error with span, got {other:?}"),
        }
    }

    #[test]
    fn test_compile_for_non_collection_union_diagnostics() {
        // array | int -> E633 NonCollectionIterable
        let Err(err_int) = crate::compiler::compile(
            "val = [1, 2]\nif true { val = 42 }\nfor x in val { x }",
            &[],
        ) else {
            panic!("array | int should fail compilation");
        };
        assert_eq!(
            err_int[0].code,
            codes::CompilerCode::NonCollectionIterable as usize
        );

        // array | null -> E633 NonCollectionIterable
        let Err(err_null) = crate::compiler::compile(
            "val = [1, 2]\nif true { val = null }\nfor x in val { x }",
            &[],
        ) else {
            panic!("array | null should fail compilation");
        };
        assert_eq!(
            err_null[0].code,
            codes::CompilerCode::NonCollectionIterable as usize
        );

        // any -> E633 NonCollectionIterable
        let Err(err_any) = crate::compiler::compile("for x in .unknown_field { x }", &[]) else {
            panic!("any should fail compilation");
        };
        assert_eq!(
            err_any[0].code,
            codes::CompilerCode::NonCollectionIterable as usize
        );

        // single var with array | object -> E634 ObjectSingleVarPattern
        let Err(err_single) = crate::compiler::compile(
            "val = [1, 2]\nif true { val = { \"a\": 1 } }\nfor x in val { x }",
            &[],
        ) else {
            panic!("single var with array | object should fail compilation");
        };
        assert_eq!(
            err_single[0].code,
            codes::CompilerCode::ObjectSingleVarPattern as usize
        );

        // key-value with array | object -> valid!
        assert!(
            crate::compiler::compile(
                "val = [1, 2]\nif true { val = { \"a\": 1 } }\nfor k, v in val { v }",
                &[],
            )
            .is_ok()
        );
    }

    #[test]
    fn test_contains_non_collection_kind() {
        assert!(!contains_non_collection_kind(&Kind::array(
            Collection::any()
        )));
        assert!(!contains_non_collection_kind(&Kind::object(
            Collection::any()
        )));
        assert!(!contains_non_collection_kind(
            &Kind::array(Collection::any()).or_object(Collection::any())
        ));

        assert!(contains_non_collection_kind(&Kind::integer()));
        assert!(contains_non_collection_kind(&Kind::bytes()));
        assert!(contains_non_collection_kind(&Kind::float()));
        assert!(contains_non_collection_kind(&Kind::boolean()));
        assert!(contains_non_collection_kind(&Kind::timestamp()));
        assert!(contains_non_collection_kind(&Kind::regex()));
        assert!(contains_non_collection_kind(&Kind::null()));
        assert!(contains_non_collection_kind(&Kind::undefined()));
        assert!(contains_non_collection_kind(&Kind::any()));
        assert!(contains_non_collection_kind(
            &Kind::array(Collection::any()).or_integer()
        ));
        assert!(contains_non_collection_kind(
            &Kind::array(Collection::any()).or_null()
        ));
    }

    #[test]
    fn control_flow_parity_between_array_and_object() {
        // Verify that break, continue, return, errors, and scope restoration
        // behave identically on arrays and objects through resolve_items
        let fns = crate::stdlib::all();
        let tz = TimeZone::default();

        let eval = |script: &str| -> (Resolved, RuntimeState) {
            let prog = crate::compiler::compile(script, &fns).unwrap();
            let mut target = TargetValue::new(Value::Null);
            let mut state = RuntimeState::default();
            let mut ctx = Context::new(&mut target, &mut state, &tz);
            let res = prog.program.resolve(&mut ctx);
            (res, state)
        };

        // 1. Break parity
        let (res_arr, _) = eval(
            "count = 0\nfor i, v in [1, 2, 3, 4] { if i == 2 { break }\ncount = count + 1 }\ncount",
        );
        let (res_obj, _) = eval(
            "count = 0\nfor k, v in { \"a\": 1, \"b\": 2, \"c\": 3, \"d\": 4 } { if v == 3 { break }\ncount = count + 1 }\ncount",
        );
        assert_eq!(res_arr.unwrap(), Value::from(2));
        assert_eq!(res_obj.unwrap(), Value::from(2));

        // 2. Continue parity
        let (res_arr, _) = eval(
            "count = 0\nfor i, v in [1, 2, 3, 4] { if i == 2 { continue }\ncount = count + 1 }\ncount",
        );
        let (res_obj, _) = eval(
            "count = 0\nfor k, v in { \"a\": 1, \"b\": 2, \"c\": 3, \"d\": 4 } { if v == 3 { continue }\ncount = count + 1 }\ncount",
        );
        assert_eq!(res_arr.unwrap(), Value::from(3));
        assert_eq!(res_obj.unwrap(), Value::from(3));

        // 3. Return parity
        let (res_arr, _) =
            eval("for v in [1, 2, 3] { if v == 2 { return \"ret_arr\" } }\n\"done\"");
        let (res_obj, _) = eval(
            "for k, v in { \"a\": 1, \"b\": 2, \"c\": 3 } { if v == 2 { return \"ret_obj\" } }\n\"done\"",
        );
        match res_arr {
            Err(ExpressionError::Return { value, .. }) => {
                assert_eq!(value, Value::from("ret_arr"));
            }
            other => panic!("expected return, got {other:?}"),
        }
        match res_obj {
            Err(ExpressionError::Return { value, .. }) => {
                assert_eq!(value, Value::from("ret_obj"));
            }
            other => panic!("expected return, got {other:?}"),
        }

        // 4. Scope restoration parity (shadowing outer variables)
        let (res_arr, _) = eval("x = \"outer\"\nfor x in [\"inner\"] { null }\nx");
        let (res_obj, _) = eval("x = \"outer\"\nfor k, x in { \"a\": \"inner\" } { null }\nx");
        assert_eq!(res_arr.unwrap(), Value::from("outer"));
        assert_eq!(res_obj.unwrap(), Value::from("outer"));

        // 5. Error propagation and scope restoration on error
        let (res_arr, state_arr) =
            eval("x = \"outer\"\nfor x in [1, 2] { if x == 2 { abort \"err\" } }");
        let (res_obj, state_obj) = eval(
            "x = \"outer\"\nfor k, x in { \"a\": 1, \"b\": 2 } { if x == 2 { abort \"err\" } }",
        );
        assert!(res_arr.is_err());
        assert_eq!(
            state_arr.variable(&Ident::new("x")),
            Some(&Value::from("outer"))
        );
        assert!(res_obj.is_err());
        assert_eq!(
            state_obj.variable(&Ident::new("x")),
            Some(&Value::from("outer"))
        );
    }
}
