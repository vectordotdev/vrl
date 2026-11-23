use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::compiler::context::BindingScopeGuard;
use crate::compiler::expression::{Block, Expr, Resolved};
use crate::compiler::parser::ast::{self, ForPattern, Ident};
use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::type_def::Details;
use crate::compiler::{Context, Expression, ExpressionError, Span, TypeDef, codes};
use crate::diagnostic::{DiagnosticMessage, Label};
use crate::path::PathPrefix;
use crate::value::kind::Collection;
use crate::value::{Kind, ObjectMap, Value};

pub(crate) const MAX_LOOP_REPASSES: usize = 128;
pub(crate) const MAX_CACHE_ENTRIES_PER_LOOP: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct For {
    pub span: Span,
    pub pattern: ForPattern,
    pub iterable: Box<Expr>,
    pub block: Block,
}

#[derive(Debug)]
enum LoopItem {
    Single(Value),
    KeyValue(Value, Value),
}

impl For {
    #[must_use]
    pub fn new(span: Span, pattern: ForPattern, iterable: Box<Expr>, block: Block) -> Self {
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
        let mut guard = BindingScopeGuard::new(ctx, first_ident, second_ident);

        for item_res in items {
            guard.ctx_mut().checkpoint()?;
            let item = item_res?;
            match item {
                LoopItem::Single(value) => guard.set_single(value),
                LoopItem::KeyValue(key, value) => guard.set_key_value(key, value),
            }

            match self.block.resolve_unused(guard.ctx_mut()) {
                Ok(()) | Err(ExpressionError::Continue { .. }) => {}
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

impl For {
    fn eval_for_body_fixed_point(
        &self,
        pre_loop_state: &TypeState,
        iterable_kind: &Kind,
        ctx: &mut LoopEvalContext,
    ) -> (TypeState, Option<TypeState>, bool, Kind, bool) {
        let mut current_state = pre_loop_state.clone();
        let body_footprint = LoopFootprint::for_block_with_pattern(&self.pattern, &self.block);
        let mut budget_widened = ctx.remaining_repasses == 0;
        if budget_widened {
            body_footprint.widen_written_state(&mut current_state);
        }

        let mut accumulated_break_state: Option<TypeState> = None;
        let mut fallible = false;
        let mut final_returns = Kind::never();
        let mut iterations = 0;

        loop {
            let mut iter_state = current_state.clone();
            let pattern_idents = bind_for_pattern(&self.pattern, iterable_kind, &mut iter_state);

            let flow = eval_loop_block_with_ctx(&self.block, &iter_state, ctx);
            fallible |= flow.result.is_fallible();
            final_returns = final_returns.union(flow.result.returns().clone());

            let back_edge_state = merge_opt_state(flow.normal, flow.continue_state).map(|mut s| {
                restore_pattern_variables(&mut s, pre_loop_state, &pattern_idents);
                canonicalize_type_state(&mut s);
                s
            });

            let iter_break_state = flow.break_state.map(|mut s| {
                restore_pattern_variables(&mut s, pre_loop_state, &pattern_idents);
                canonicalize_type_state(&mut s);
                s
            });
            accumulated_break_state = merge_opt_state(accumulated_break_state, iter_break_state);
            if let Some(ref mut brk) = accumulated_break_state {
                canonicalize_type_state(brk);
            }

            let mut next_state = match back_edge_state {
                Some(back_state) => current_state.clone().merge(back_state),
                None => current_state.clone(),
            };
            canonicalize_type_state(&mut next_state);

            if next_state == current_state || budget_widened {
                current_state = next_state;
                break;
            }

            if ctx.remaining_repasses == 0 {
                body_footprint.widen_written_state(&mut next_state);
                current_state = next_state;
                budget_widened = true;
                continue;
            }
            ctx.remaining_repasses -= 1;

            iterations += 1;
            if iterations >= WIDEN_AFTER_ITERATIONS {
                widen_differing_states(
                    &current_state,
                    &mut next_state,
                    iterations >= FALLBACK_WIDEN_ITERATIONS,
                );
            }
            current_state = next_state;
        }

        (
            current_state,
            accumulated_break_state,
            fallible,
            final_returns,
            budget_widened,
        )
    }

    pub(crate) fn eval_for_in_loop_with_ctx(
        &self,
        state: &TypeState,
        ctx: &mut LoopEvalContext,
    ) -> (TypeState, LoopFlow) {
        let footprint = LoopFootprint::for_expr(self);
        let projected_input = (!footprint.opaque).then(|| footprint.project(state));

        if let Some(ref input) = projected_input
            && let Some(cached) = ctx.lookup(self.span, input)
        {
            return cached.apply(state);
        }

        let iter_flow = eval_loop_expr_with_ctx(&self.iterable, state, ctx);
        let Some(mut pre_loop_state) = iter_flow.normal else {
            let mut fallback_header = self.iterable.type_info(state).state;
            canonicalize_type_state(&mut fallback_header);
            return (
                fallback_header,
                LoopFlow::never_with_exits(
                    iter_flow.break_state,
                    iter_flow.continue_state,
                    iter_flow.result.is_fallible(),
                    iter_flow.result.returns().clone(),
                ),
            );
        };

        canonicalize_type_state(&mut pre_loop_state);
        let (header_state, accumulated_break_state, body_fallible, body_returns, budget_widened) =
            self.eval_for_body_fixed_point(&pre_loop_state, iter_flow.result.kind(), ctx);

        let mut post_loop_normal =
            merge_opt_state(Some(header_state.clone()), accumulated_break_state)
                .expect("at least header_state")
                .merge(pre_loop_state);
        canonicalize_type_state(&mut post_loop_normal);

        let result_flow = LoopFlow {
            normal: Some(post_loop_normal),
            break_state: iter_flow.break_state,
            continue_state: iter_flow.continue_state,
            result: TypeDef::null()
                .maybe_fallible(iter_flow.result.is_fallible() || body_fallible)
                .with_returns(iter_flow.result.returns().clone().union(body_returns)),
        };

        if let Some(input) = projected_input
            && !budget_widened
        {
            ctx.insert(
                self.span,
                input,
                CachedLoopEntry::from_result(&footprint, &header_state, &result_flow),
            );
        }

        (header_state, result_flow)
    }

    pub(crate) fn resolve_loop_type_info(&self, state: &TypeState) -> (TypeState, TypeInfo) {
        let mut ctx = LoopEvalContext::new();
        self.resolve_loop_type_info_with_ctx(state, &mut ctx)
    }

    pub(crate) fn resolve_loop_type_info_with_ctx(
        &self,
        state: &TypeState,
        ctx: &mut LoopEvalContext,
    ) -> (TypeState, TypeInfo) {
        let (header_state, flow) = self.eval_for_in_loop_with_ctx(state, ctx);
        let mut final_state = merge_opt_state(
            flow.normal,
            merge_opt_state(flow.break_state, flow.continue_state),
        )
        .unwrap_or_else(|| header_state.clone());
        canonicalize_type_state(&mut final_state);
        (header_state, TypeInfo::new(final_state, flow.result))
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
        self.resolve_loop_type_info(state).1
    }
}

pub(crate) struct LoopFlow {
    pub(crate) normal: Option<TypeState>,
    pub(crate) break_state: Option<TypeState>,
    pub(crate) continue_state: Option<TypeState>,
    pub(crate) result: TypeDef,
}

impl LoopFlow {
    fn from_type_info(info: TypeInfo) -> Self {
        Self {
            normal: (!info.result.is_never()).then_some(info.state),
            break_state: None,
            continue_state: None,
            result: info.result,
        }
    }

    fn never_with_exits(
        break_state: Option<TypeState>,
        continue_state: Option<TypeState>,
        fallible: bool,
        returns: Kind,
    ) -> Self {
        Self {
            normal: None,
            break_state,
            continue_state,
            result: TypeDef::never()
                .maybe_fallible(fallible)
                .with_returns(returns),
        }
    }

    fn merge_branch(
        self,
        other: Self,
        combine_result: impl FnOnce(TypeDef, TypeDef) -> TypeDef,
    ) -> Self {
        Self {
            normal: merge_opt_state(self.normal, other.normal),
            break_state: merge_opt_state(self.break_state, other.break_state),
            continue_state: merge_opt_state(self.continue_state, other.continue_state),
            result: combine_result(self.result, other.result),
        }
    }

    fn with_prefix(mut self, prefix: Self) -> Self {
        if prefix.result.is_fallible() {
            self.result = self.result.fallible();
        }
        self.result
            .returns_mut()
            .merge_keep(prefix.result.returns().clone(), false);
        self.break_state = merge_opt_state(prefix.break_state, self.break_state);
        self.continue_state = merge_opt_state(prefix.continue_state, self.continue_state);
        self
    }

    fn with_result(mut self, result: TypeDef) -> Self {
        self.result = result;
        self
    }

    fn diverge(self, extra_return: Option<Kind>) -> Self {
        let mut returns = self.result.returns().clone();
        if let Some(kind) = extra_return {
            returns.merge_keep(kind, false);
        }
        Self::never_with_exits(
            self.break_state,
            self.continue_state,
            self.result.is_fallible(),
            returns,
        )
    }
}

struct LoopFlowSeq {
    normal: Option<TypeState>,
    break_state: Option<TypeState>,
    continue_state: Option<TypeState>,
    fallible: bool,
    returns: Kind,
}

impl LoopFlowSeq {
    fn new(state: &TypeState) -> Self {
        Self {
            normal: Some(state.clone()),
            break_state: None,
            continue_state: None,
            fallible: false,
            returns: Kind::never(),
        }
    }

    fn step_with_ctx(
        &mut self,
        expr: &Expr,
        track_fallibility: bool,
        ctx: &mut LoopEvalContext,
    ) -> Option<TypeDef> {
        let active_state = self.normal.as_ref()?;
        let flow = eval_loop_expr_with_ctx(expr, active_state, ctx);
        if track_fallibility && flow.result.is_fallible() {
            self.fallible = true;
        }
        self.returns
            .merge_keep(flow.result.returns().clone(), false);
        self.normal = flow.normal;
        self.break_state = merge_opt_state(self.break_state.take(), flow.break_state);
        self.continue_state = merge_opt_state(self.continue_state.take(), flow.continue_state);
        Some(flow.result)
    }

    fn finish(self, result: TypeDef) -> LoopFlow {
        LoopFlow {
            normal: self.normal,
            break_state: self.break_state,
            continue_state: self.continue_state,
            result: result
                .maybe_fallible(self.fallible)
                .with_returns(self.returns),
        }
    }

    fn finish_never(self) -> LoopFlow {
        LoopFlow::never_with_exits(
            self.break_state,
            self.continue_state,
            self.fallible,
            self.returns,
        )
    }
}

fn merge_opt_state(a: Option<TypeState>, b: Option<TypeState>) -> Option<TypeState> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.merge(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn expr_contains_loop_jump(expr: &Expr) -> bool {
    expr_matches_control_flow(expr, false, true)
}

pub(crate) fn expr_contains_for(expr: &Expr) -> bool {
    expr_matches_control_flow(expr, true, false)
}

fn expr_needs_loop_eval(expr: &Expr) -> bool {
    expr_matches_control_flow(expr, true, true)
}

fn expr_matches_control_flow(expr: &Expr, include_nested_for: bool, include_exits: bool) -> bool {
    use crate::compiler::expression::{container::Variant, query::Target, unary};

    match expr {
        Expr::Break(_) | Expr::Continue(_) | Expr::Abort(_) | Expr::Return(_) => include_exits,
        // A nested `for` loop captures jumps from its body, but jumps inside its
        // `iterable` still target the enclosing `for` loop. When evaluating with a
        // shared `LoopEvalContext`, nested `for` loops must also go through
        // `eval_loop_expr_with_ctx` so they share the summary cache and pass budget.
        Expr::For(for_expr) => {
            include_nested_for
                || expr_matches_control_flow(&for_expr.iterable, false, include_exits)
        }
        // Neither function arguments nor closures may break/continue an enclosing `for` loop.
        Expr::Literal(_) | Expr::Variable(_) | Expr::Noop(_) | Expr::FunctionCall(_) => false,
        Expr::Container(container) => match &container.variant {
            Variant::Group(group) => {
                expr_matches_control_flow(group.inner_expr(), include_nested_for, include_exits)
            }
            Variant::Block(block) => block
                .exprs()
                .iter()
                .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits)),
            Variant::Array(array) => array
                .iter()
                .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits)),
            Variant::Object(object) => object
                .values()
                .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits)),
        },
        Expr::IfStatement(if_stmt) => {
            if_stmt
                .predicate
                .block()
                .exprs()
                .iter()
                .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits))
                || if_stmt
                    .if_block
                    .exprs()
                    .iter()
                    .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits))
                || if_stmt.else_block.as_ref().is_some_and(|b| {
                    b.exprs()
                        .iter()
                        .any(|e| expr_matches_control_flow(e, include_nested_for, include_exits))
                })
        }
        Expr::Op(op) => {
            expr_matches_control_flow(&op.lhs, include_nested_for, include_exits)
                || expr_matches_control_flow(&op.rhs, include_nested_for, include_exits)
        }
        Expr::Assignment(assignment) => match assignment.variant() {
            crate::compiler::expression::assignment::Variant::Single { expr, .. }
            | crate::compiler::expression::assignment::Variant::Infallible { expr, .. } => {
                expr_matches_control_flow(expr, include_nested_for, include_exits)
            }
        },
        Expr::Query(query) => match query.target() {
            Target::Internal(_) | Target::External(_) | Target::FunctionCall(_) => false,
            Target::Container(container) => expr_matches_control_flow(
                &Expr::Container(container.clone()),
                include_nested_for,
                include_exits,
            ),
        },
        Expr::Unary(unary) => match unary.variant() {
            unary::Variant::Not(not) => {
                expr_matches_control_flow(not.inner_expr(), include_nested_for, include_exits)
            }
        },
    }
}

fn eval_loop_block_with_ctx(
    block: &Block,
    state: &TypeState,
    ctx: &mut LoopEvalContext,
) -> LoopFlow {
    if !block.exprs().iter().any(expr_needs_loop_eval) {
        return LoopFlow::from_type_info(block.type_info(state));
    }

    let parent_locals = state.local.clone();
    let mut seq = LoopFlowSeq::new(state);
    let mut result = TypeDef::null();
    let mut after_never_expression = false;

    for expr in block.exprs() {
        let Some(expr_result) = seq.step_with_ctx(expr, !after_never_expression, ctx) else {
            break;
        };
        if expr_result.is_never() {
            after_never_expression = true;
        }
        result = expr_result;
    }

    let mut flow = seq.finish(result);
    if block.is_scoped() {
        if let Some(ref mut normal) = flow.normal {
            normal.local = parent_locals
                .clone()
                .apply_child_scope(normal.local.clone());
        }
        if let Some(ref mut brk) = flow.break_state {
            brk.local = parent_locals.clone().apply_child_scope(brk.local.clone());
        }
        if let Some(ref mut cont) = flow.continue_state {
            cont.local = parent_locals.apply_child_scope(cont.local.clone());
        }
    }
    flow
}

#[allow(clippy::too_many_lines)]
pub(crate) fn eval_loop_expr_with_ctx(
    expr: &Expr,
    state: &TypeState,
    ctx: &mut LoopEvalContext,
) -> LoopFlow {
    use crate::compiler::expression::{assignment, container::Variant, query::Target, unary};
    use crate::parser::ast::Opcode;

    if !expr_needs_loop_eval(expr) {
        return LoopFlow::from_type_info(expr.type_info(state));
    }

    match expr {
        Expr::Break(_) => {
            LoopFlow::never_with_exits(Some(state.clone()), None, false, Kind::never())
        }
        Expr::Continue(_) => {
            LoopFlow::never_with_exits(None, Some(state.clone()), false, Kind::never())
        }
        Expr::For(for_expr) => for_expr.eval_for_in_loop_with_ctx(state, ctx).1,
        Expr::Container(container) => match &container.variant {
            Variant::Group(group) => eval_loop_expr_with_ctx(group.inner_expr(), state, ctx),
            Variant::Block(block) => eval_loop_block_with_ctx(block, state, ctx),
            Variant::Array(array) => {
                let mut seq = LoopFlowSeq::new(state);
                let mut type_defs = Vec::with_capacity(array.len());

                for item in array.iter() {
                    let Some(type_def) = seq.step_with_ctx(item, true, ctx) else {
                        break;
                    };
                    let type_def = type_def.upgrade_undefined();
                    if type_def.is_never() || seq.normal.is_none() {
                        return seq.finish_never();
                    }
                    type_defs.push(type_def);
                }

                let collection = type_defs
                    .into_iter()
                    .enumerate()
                    .map(|(index, type_def)| (index.into(), type_def.into()))
                    .collect::<BTreeMap<_, _>>();
                seq.finish(TypeDef::array(collection))
            }
            Variant::Object(object) => {
                let mut seq = LoopFlowSeq::new(state);
                let mut type_defs = BTreeMap::new();

                for (k, item) in object.iter() {
                    let Some(type_def) = seq.step_with_ctx(item, true, ctx) else {
                        break;
                    };
                    let type_def = type_def.upgrade_undefined();
                    if type_def.is_never() || seq.normal.is_none() {
                        return seq.finish_never();
                    }
                    type_defs.insert(k.clone(), type_def);
                }

                let collection = type_defs
                    .into_iter()
                    .map(|(field, type_def)| (field.into(), type_def.into()))
                    .collect::<BTreeMap<_, _>>();
                seq.finish(TypeDef::object(collection))
            }
        },
        Expr::IfStatement(if_stmt) => {
            let mut pred_flow = eval_loop_block_with_ctx(if_stmt.predicate.block(), state, ctx);
            let Some(after_pred_state) = pred_flow.normal.take() else {
                return pred_flow;
            };

            let if_flow = eval_loop_block_with_ctx(&if_stmt.if_block, &after_pred_state, ctx);
            let branch_flow = if let Some(else_block) = &if_stmt.else_block {
                let else_flow = eval_loop_block_with_ctx(else_block, &after_pred_state, ctx);
                if_flow.merge_branch(else_flow, TypeDef::union)
            } else {
                let fallthrough = LoopFlow {
                    normal: Some(after_pred_state),
                    break_state: None,
                    continue_state: None,
                    result: TypeDef::null(),
                };
                if_flow.merge_branch(fallthrough, |if_res, _| if_res.or_null())
            };
            branch_flow.with_prefix(pred_flow)
        }
        Expr::Assignment(assignment) => match assignment.variant() {
            assignment::Variant::Single { target, expr } => {
                let mut rhs_flow = eval_loop_expr_with_ctx(expr, state, ctx);
                let expr_result = rhs_flow.result.clone().impure();
                if let Some(ref mut s) = rhs_flow.normal {
                    let const_value = expr.resolve_constant(s);
                    target.insert_type_def(s, expr_result.clone(), const_value);
                }
                rhs_flow.with_result(expr_result)
            }
            assignment::Variant::Infallible {
                ok,
                err,
                expr,
                default,
            } => {
                let mut rhs_flow = eval_loop_expr_with_ctx(expr, state, ctx);
                if let Some(ref mut s) = rhs_flow.normal {
                    let expr_result = rhs_flow.result.clone();
                    let ok_type = expr_result
                        .clone()
                        .union(TypeDef::from(default.kind()))
                        .infallible();
                    let err_type = TypeDef::from(Kind::bytes().or_null());
                    let assignment_result = expr_result.infallible().impure().or_bytes();
                    let const_value = expr.resolve_constant(s);
                    ok.insert_type_def(s, ok_type, const_value);
                    err.insert_type_def(s, err_type, None);
                    rhs_flow.with_result(assignment_result)
                } else {
                    rhs_flow
                }
            }
        },
        Expr::Op(op) => {
            let lhs_flow = eval_loop_expr_with_ctx(&op.lhs, state, ctx);
            let lhs_def = lhs_flow.result.clone();
            let lhs_value = op.lhs.resolve_constant(state);
            let Some(lhs_state) = lhs_flow.normal else {
                return lhs_flow;
            };

            let normal_from_lhs = match op.opcode {
                Opcode::Err => Some(lhs_state.clone()),
                Opcode::Or => {
                    if lhs_def.is_null() || lhs_value == Some(Value::Boolean(false)) {
                        None
                    } else if !(lhs_def.contains_null() || lhs_def.contains_boolean())
                        || lhs_value == Some(Value::Boolean(true))
                    {
                        return LoopFlow {
                            normal: Some(lhs_state),
                            break_state: lhs_flow.break_state,
                            continue_state: lhs_flow.continue_state,
                            result: lhs_def,
                        };
                    } else {
                        Some(lhs_state.clone())
                    }
                }
                Opcode::And => {
                    if lhs_def.is_null() || lhs_value == Some(Value::Boolean(false)) {
                        return LoopFlow {
                            normal: Some(lhs_state),
                            break_state: lhs_flow.break_state,
                            continue_state: lhs_flow.continue_state,
                            result: TypeDef::boolean()
                                .maybe_fallible(lhs_def.is_fallible())
                                .with_returns(lhs_def.returns().clone()),
                        };
                    } else if lhs_value == Some(Value::Boolean(true)) {
                        None
                    } else {
                        Some(lhs_state.clone())
                    }
                }
                _ => None,
            };

            let rhs_input_state = if op.opcode == Opcode::Err {
                state.clone().merge(lhs_state)
            } else {
                lhs_state
            };
            let rhs_value = op.rhs.resolve_constant(&rhs_input_state);
            let rhs_flow = eval_loop_expr_with_ctx(&op.rhs, &rhs_input_state, ctx);
            let rhs_def = rhs_flow.result;
            let normal = merge_opt_state(normal_from_lhs, rhs_flow.normal);
            let result = if normal.is_some() {
                op.compute_result_type(lhs_def, lhs_value.as_ref(), rhs_def, rhs_value.as_ref())
            } else {
                let mut returns = lhs_def.returns().clone();
                returns.merge_keep(rhs_def.returns().clone(), false);
                TypeDef::never()
                    .maybe_fallible(lhs_def.is_fallible() || rhs_def.is_fallible())
                    .with_returns(returns)
            };
            LoopFlow {
                normal,
                break_state: merge_opt_state(lhs_flow.break_state, rhs_flow.break_state),
                continue_state: merge_opt_state(lhs_flow.continue_state, rhs_flow.continue_state),
                result,
            }
        }
        Expr::Query(query) => match query.target() {
            Target::Container(container) => {
                let mut flow =
                    eval_loop_expr_with_ctx(&Expr::Container(container.clone()), state, ctx);
                if flow.normal.is_some() {
                    flow.result = flow.result.at_path(query.path());
                }
                flow
            }
            Target::Internal(_) | Target::External(_) | Target::FunctionCall(_) => unreachable!(),
        },
        Expr::Unary(unary) => match unary.variant() {
            unary::Variant::Not(not) => {
                let inner_flow = eval_loop_expr_with_ctx(not.inner_expr(), state, ctx);
                let base_result = if inner_flow.normal.is_some() {
                    TypeDef::boolean()
                } else {
                    TypeDef::never()
                };
                let result = base_result
                    .maybe_fallible(inner_flow.result.is_fallible())
                    .with_returns(inner_flow.result.returns().clone());
                inner_flow.with_result(result)
            }
        },
        Expr::Abort(abort) => match abort.message() {
            Some(msg) => eval_loop_expr_with_ctx(msg, state, ctx).diverge(None),
            None => LoopFlow::never_with_exits(None, None, false, Kind::never()),
        },
        Expr::Return(ret) => {
            let inner_flow = eval_loop_expr_with_ctx(ret.expr(), state, ctx);
            let extra_return = inner_flow
                .normal
                .is_some()
                .then(|| inner_flow.result.kind().clone());
            inner_flow.diverge(extra_return)
        }
        Expr::Literal(_) | Expr::Variable(_) | Expr::Noop(_) | Expr::FunctionCall(_) => {
            unreachable!()
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ProjectedLoopState {
    locals: BTreeMap<Ident, Details>,
    event: Option<Details>,
    metadata: Option<Kind>,
}

#[derive(Clone)]
pub(crate) struct LoopStatePatch {
    locals: BTreeMap<Ident, Details>,
    event: Option<Details>,
    metadata: Option<Kind>,
}

impl LoopStatePatch {
    pub(crate) fn apply(&self, base: &TypeState) -> TypeState {
        let mut out = base.clone();
        for (ident, details) in &self.locals {
            out.local.insert_variable(ident.clone(), details.clone());
        }
        if let Some(ref event) = self.event {
            out.external.update_target(event.clone());
        }
        if let Some(ref metadata) = self.metadata {
            out.external.update_metadata(metadata.clone());
        }
        canonicalize_type_state(&mut out);
        out
    }
}

struct CachedLoopEntry {
    header: LoopStatePatch,
    normal: Option<LoopStatePatch>,
    break_state: Option<LoopStatePatch>,
    continue_state: Option<LoopStatePatch>,
    result: TypeDef,
}

impl CachedLoopEntry {
    fn from_result(footprint: &LoopFootprint, header: &TypeState, flow: &LoopFlow) -> Self {
        Self {
            header: footprint.patch_from(header),
            normal: flow.normal.as_ref().map(|s| footprint.patch_from(s)),
            break_state: flow.break_state.as_ref().map(|s| footprint.patch_from(s)),
            continue_state: flow
                .continue_state
                .as_ref()
                .map(|s| footprint.patch_from(s)),
            result: flow.result.clone(),
        }
    }

    fn apply(&self, base: &TypeState) -> (TypeState, LoopFlow) {
        (
            self.header.apply(base),
            LoopFlow {
                normal: self.normal.as_ref().map(|p| p.apply(base)),
                break_state: self.break_state.as_ref().map(|p| p.apply(base)),
                continue_state: self.continue_state.as_ref().map(|p| p.apply(base)),
                result: self.result.clone(),
            },
        )
    }
}

pub(crate) struct LoopEvalContext {
    remaining_repasses: usize,
    cache: HashMap<Span, Vec<(ProjectedLoopState, CachedLoopEntry)>>,
}

impl LoopEvalContext {
    pub(crate) fn new() -> Self {
        Self::with_budget(MAX_LOOP_REPASSES)
    }

    pub(crate) fn with_budget(remaining_repasses: usize) -> Self {
        Self {
            remaining_repasses,
            cache: HashMap::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.remaining_repasses = MAX_LOOP_REPASSES;
        self.cache.clear();
    }

    pub(crate) fn invalidate(&mut self, span: Span) {
        self.cache.remove(&span);
    }

    pub(crate) const fn is_exhausted(&self) -> bool {
        self.remaining_repasses == 0
    }

    pub(crate) fn consume_repass(&mut self) -> bool {
        if self.remaining_repasses == 0 {
            false
        } else {
            self.remaining_repasses -= 1;
            true
        }
    }

    fn lookup(&self, span: Span, input: &ProjectedLoopState) -> Option<&CachedLoopEntry> {
        self.cache
            .get(&span)?
            .iter()
            .find_map(|(cached_in, out)| (cached_in == input).then_some(out))
    }

    fn insert(&mut self, span: Span, input: ProjectedLoopState, entry: CachedLoopEntry) {
        let bucket = self.cache.entry(span).or_default();
        if bucket.len() < MAX_CACHE_ENTRIES_PER_LOOP && !bucket.iter().any(|(k, _)| k == &input) {
            bucket.push((input, entry));
        }
    }
}

#[derive(Default, Clone, Copy)]
struct ExternalAccess {
    reads: bool,
    writes: bool,
}

impl ExternalAccess {
    fn mark_read(&mut self) {
        self.reads = true;
    }

    fn mark_read_write(&mut self) {
        self.reads = true;
        self.writes = true;
    }

    const fn touched(self) -> bool {
        self.reads || self.writes
    }
}

#[derive(Default)]
pub(crate) struct LoopFootprint {
    read_locals: BTreeSet<Ident>,
    written_locals: BTreeSet<Ident>,
    event: ExternalAccess,
    metadata: ExternalAccess,
    pub(crate) opaque: bool,
}

impl LoopFootprint {
    pub(crate) fn for_expr(for_expr: &For) -> Self {
        let mut fp = Self::default();
        let mut shadowed = Vec::new();
        fp.visit_expr(&for_expr.iterable, &mut shadowed);
        fp.visit_for_body_expr(&for_expr.pattern, &for_expr.block, &mut shadowed);
        fp
    }

    fn for_block_with_pattern(pattern: &ForPattern, block: &Block) -> Self {
        let mut fp = Self::default();
        let mut shadowed = Vec::new();
        fp.visit_for_body_expr(pattern, block, &mut shadowed);
        fp
    }

    pub(crate) fn for_ast(for_stmt: &ast::ForStatement) -> Self {
        let mut fp = Self::default();
        let mut shadowed = Vec::new();
        fp.visit_ast_expr(for_stmt.iterable.inner(), &mut shadowed);
        fp.visit_for_body_ast(&for_stmt.pattern, for_stmt.block.inner(), &mut shadowed);
        fp
    }

    pub(crate) fn for_ast_body(pattern: &ForPattern, block: &ast::Block) -> Self {
        let mut fp = Self::default();
        let mut shadowed = Vec::new();
        fp.visit_for_body_ast(pattern, block, &mut shadowed);
        fp
    }

    pub(crate) fn project(&self, state: &TypeState) -> ProjectedLoopState {
        let mut locals = BTreeMap::new();
        for ident in self.read_locals.union(&self.written_locals) {
            if let Some(details) = state.local.variable(ident) {
                locals.insert(ident.clone(), details.clone());
            }
        }
        let event = self
            .event
            .touched()
            .then(|| state.external.target().clone());
        let metadata = self
            .metadata
            .touched()
            .then(|| state.external.metadata_kind().clone());
        ProjectedLoopState {
            locals,
            event,
            metadata,
        }
    }

    pub(crate) fn patch_from(&self, state: &TypeState) -> LoopStatePatch {
        let mut locals = BTreeMap::new();
        for ident in &self.written_locals {
            if let Some(details) = state.local.variable(ident) {
                locals.insert(ident.clone(), details.clone());
            }
        }
        let event = self.event.writes.then(|| state.external.target().clone());
        let metadata = self
            .metadata
            .writes
            .then(|| state.external.metadata_kind().clone());
        LoopStatePatch {
            locals,
            event,
            metadata,
        }
    }

    pub(crate) fn widen_written_state(&self, state: &mut TypeState) {
        let top = Details {
            type_def: TypeDef::any().impure(),
            value: None,
        };
        if self.opaque {
            for details in state.local.bindings.values_mut() {
                *details = top.clone();
            }
            state.external.update_target(top);
            state.external.update_metadata(Kind::any());
            canonicalize_type_state(state);
            return;
        }
        for ident in &self.written_locals {
            if let Some(details) = state.local.bindings.get_mut(ident) {
                *details = top.clone();
            }
        }
        if self.event.writes {
            state.external.update_target(top);
        }
        if self.metadata.writes {
            state.external.update_metadata(Kind::any());
        }
        canonicalize_type_state(state);
    }

    fn record_read_local(&mut self, ident: &Ident, shadowed: &[Ident]) {
        if !is_wildcard(ident) && !shadowed.contains(ident) {
            self.read_locals.insert(ident.clone());
        }
    }

    fn record_write_local(&mut self, ident: &Ident, also_read: bool, shadowed: &[Ident]) {
        if !is_wildcard(ident) && !shadowed.contains(ident) {
            self.written_locals.insert(ident.clone());
            if also_read {
                self.read_locals.insert(ident.clone());
            }
        }
    }

    fn push_pattern_shadows(pattern: &ForPattern, shadowed: &mut Vec<Ident>) -> usize {
        let saved_len = shadowed.len();
        match pattern {
            ForPattern::Single(val) => {
                if !is_wildcard(val.inner()) {
                    shadowed.push(val.inner().clone());
                }
            }
            ForPattern::KeyValue(key, val) => {
                if !is_wildcard(key.inner()) {
                    shadowed.push(key.inner().clone());
                }
                if !is_wildcard(val.inner()) {
                    shadowed.push(val.inner().clone());
                }
            }
        }
        saved_len
    }

    fn visit_for_body_expr(
        &mut self,
        pattern: &ForPattern,
        block: &Block,
        shadowed: &mut Vec<Ident>,
    ) {
        let saved_len = Self::push_pattern_shadows(pattern, shadowed);
        for expr in block.exprs() {
            self.visit_expr(expr, shadowed);
        }
        shadowed.truncate(saved_len);
    }

    fn visit_for_body_ast(
        &mut self,
        pattern: &ForPattern,
        block: &ast::Block,
        shadowed: &mut Vec<Ident>,
    ) {
        let saved_len = Self::push_pattern_shadows(pattern, shadowed);
        for expr in &block.0 {
            self.visit_ast_expr(expr.inner(), shadowed);
        }
        shadowed.truncate(saved_len);
    }

    fn visit_expr_target(
        &mut self,
        target: &crate::compiler::expression::assignment::Target,
        shadowed: &[Ident],
    ) {
        use crate::compiler::expression::assignment::Target;

        match target {
            Target::Noop => {}
            Target::Internal(ident, path) => {
                self.record_write_local(ident, !path.is_root(), shadowed);
            }
            Target::External(path) => match path.prefix {
                PathPrefix::Event => self.event.mark_read_write(),
                PathPrefix::Metadata => self.metadata.mark_read_write(),
            },
        }
    }

    fn visit_expr_function_call(
        &mut self,
        call: &crate::compiler::expression::FunctionCall,
        shadowed: &mut Vec<Ident>,
    ) {
        self.event.mark_read_write();
        self.metadata.mark_read_write();
        if call.is_speculative_placeholder() {
            self.opaque = true;
            return;
        }
        for arg in call.arguments() {
            self.visit_expr(arg.inner().expr(), shadowed);
        }
        if let Some(closure) = call.closure() {
            let saved_len = shadowed.len();
            for ident in &closure.variables {
                if !is_wildcard(ident) {
                    shadowed.push(ident.clone());
                }
            }
            for expr in closure.block.exprs() {
                self.visit_expr(expr, shadowed);
            }
            shadowed.truncate(saved_len);
        }
    }

    fn visit_expr(&mut self, expr: &Expr, shadowed: &mut Vec<Ident>) {
        use crate::compiler::expression::{assignment, container::Variant, query::Target, unary};

        match expr {
            Expr::Literal(_) | Expr::Noop(_) | Expr::Break(_) | Expr::Continue(_) => {}
            Expr::Variable(var) => self.record_read_local(var.ident(), shadowed),
            Expr::For(for_expr) => {
                self.visit_expr(&for_expr.iterable, shadowed);
                self.visit_for_body_expr(&for_expr.pattern, &for_expr.block, shadowed);
            }
            Expr::Container(container) => match &container.variant {
                Variant::Group(group) => self.visit_expr(group.inner_expr(), shadowed),
                Variant::Block(block) => {
                    for e in block.exprs() {
                        self.visit_expr(e, shadowed);
                    }
                }
                Variant::Array(array) => {
                    for e in array.iter() {
                        self.visit_expr(e, shadowed);
                    }
                }
                Variant::Object(object) => {
                    for e in object.values() {
                        self.visit_expr(e, shadowed);
                    }
                }
            },
            Expr::IfStatement(if_stmt) => {
                for e in if_stmt.predicate.block().exprs() {
                    self.visit_expr(e, shadowed);
                }
                for e in if_stmt.if_block.exprs() {
                    self.visit_expr(e, shadowed);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for e in else_block.exprs() {
                        self.visit_expr(e, shadowed);
                    }
                }
            }
            Expr::Op(op) => {
                self.visit_expr(&op.lhs, shadowed);
                self.visit_expr(&op.rhs, shadowed);
            }
            Expr::Assignment(assignment) => match assignment.variant() {
                assignment::Variant::Single { target, expr } => {
                    self.visit_expr(expr, shadowed);
                    self.visit_expr_target(target, shadowed);
                }
                assignment::Variant::Infallible { ok, err, expr, .. } => {
                    self.visit_expr(expr, shadowed);
                    self.visit_expr_target(ok, shadowed);
                    self.visit_expr_target(err, shadowed);
                }
            },
            Expr::Query(query) => match query.target() {
                Target::Internal(var) => self.record_read_local(var.ident(), shadowed),
                Target::External(PathPrefix::Event) => self.event.mark_read(),
                Target::External(PathPrefix::Metadata) => self.metadata.mark_read(),
                Target::Container(container) => {
                    self.visit_expr(&Expr::Container(container.clone()), shadowed);
                }
                Target::FunctionCall(call) => self.visit_expr_function_call(call, shadowed),
            },
            Expr::FunctionCall(call) => self.visit_expr_function_call(call, shadowed),
            Expr::Unary(unary) => match unary.variant() {
                unary::Variant::Not(not) => self.visit_expr(not.inner_expr(), shadowed),
            },
            Expr::Abort(abort) => {
                if let Some(msg) = abort.message() {
                    self.visit_expr(msg, shadowed);
                }
            }
            Expr::Return(ret) => self.visit_expr(ret.expr(), shadowed),
        }
    }

    fn visit_ast_target(
        &mut self,
        target: &ast::AssignmentTarget,
        is_merge: bool,
        shadowed: &mut Vec<Ident>,
    ) {
        match target {
            ast::AssignmentTarget::Noop => {}
            ast::AssignmentTarget::Internal(ident, path) => {
                self.record_write_local(ident, is_merge || path.is_some(), shadowed);
            }
            ast::AssignmentTarget::External(path) => {
                let prefix = path
                    .as_ref()
                    .map_or(PathPrefix::Event, |target_path| target_path.prefix);
                match prefix {
                    PathPrefix::Event => self.event.mark_read_write(),
                    PathPrefix::Metadata => self.metadata.mark_read_write(),
                }
            }
            ast::AssignmentTarget::Query(query) => match query.target.inner() {
                ast::QueryTarget::Internal(ident) => {
                    self.record_write_local(ident, true, shadowed);
                }
                ast::QueryTarget::External(PathPrefix::Event) => self.event.mark_read_write(),
                ast::QueryTarget::External(PathPrefix::Metadata) => self.metadata.mark_read_write(),
                ast::QueryTarget::Container(c) => self.visit_ast_container(c, shadowed),
                ast::QueryTarget::FunctionCall(fc) => self.visit_ast_function_call(fc, shadowed),
            },
        }
    }

    fn visit_ast_container(&mut self, container: &ast::Container, shadowed: &mut Vec<Ident>) {
        match container {
            ast::Container::Group(group) => self.visit_ast_expr(group.inner().0.inner(), shadowed),
            ast::Container::Block(block) => {
                for expr in &block.inner().0 {
                    self.visit_ast_expr(expr.inner(), shadowed);
                }
            }
            ast::Container::Array(array) => {
                for expr in &array.inner().0 {
                    self.visit_ast_expr(expr.inner(), shadowed);
                }
            }
            ast::Container::Object(object) => {
                for expr in object.inner().0.values() {
                    self.visit_ast_expr(expr.inner(), shadowed);
                }
            }
        }
    }

    fn visit_ast_function_call(&mut self, call: &ast::FunctionCall, shadowed: &mut Vec<Ident>) {
        self.event.mark_read_write();
        self.metadata.mark_read_write();
        for arg in &call.arguments {
            self.visit_ast_expr(arg.inner().expr.inner(), shadowed);
        }
        if let Some(closure) = &call.closure {
            let saved_len = shadowed.len();
            for var in &closure.inner().variables {
                if !is_wildcard(var.inner()) {
                    shadowed.push(var.inner().clone());
                }
            }
            for expr in &closure.inner().block.inner().0 {
                self.visit_ast_expr(expr.inner(), shadowed);
            }
            shadowed.truncate(saved_len);
        }
    }

    fn visit_ast_expr(&mut self, expr: &ast::Expr, shadowed: &mut Vec<Ident>) {
        use crate::parser::template_string::StringSegment;

        match expr {
            ast::Expr::Literal(lit) => {
                if let ast::Literal::String(template) = lit.inner() {
                    for seg in &template.0 {
                        if let StringSegment::Template(name, _) = seg {
                            self.record_read_local(&Ident::from(name.clone()), shadowed);
                        }
                    }
                }
            }
            ast::Expr::Break(_) | ast::Expr::Continue(_) => {}
            ast::Expr::Variable(ident) => self.record_read_local(ident.inner(), shadowed),
            ast::Expr::Container(c) => self.visit_ast_container(c.inner(), shadowed),
            ast::Expr::IfStatement(if_stmt) => {
                match if_stmt.inner().predicate.inner() {
                    ast::Predicate::One(e) => self.visit_ast_expr(e.inner(), shadowed),
                    ast::Predicate::Many(es) => {
                        for e in es {
                            self.visit_ast_expr(e.inner(), shadowed);
                        }
                    }
                }
                for e in &if_stmt.inner().if_node.inner().0 {
                    self.visit_ast_expr(e.inner(), shadowed);
                }
                if let Some(else_node) = &if_stmt.inner().else_node {
                    for e in &else_node.inner().0 {
                        self.visit_ast_expr(e.inner(), shadowed);
                    }
                }
            }
            ast::Expr::For(for_stmt) => {
                self.visit_ast_expr(for_stmt.inner().iterable.inner(), shadowed);
                self.visit_for_body_ast(
                    &for_stmt.inner().pattern,
                    for_stmt.inner().block.inner(),
                    shadowed,
                );
            }
            ast::Expr::Op(op) => {
                self.visit_ast_expr(op.inner().0.inner(), shadowed);
                self.visit_ast_expr(op.inner().2.inner(), shadowed);
            }
            ast::Expr::Assignment(assign) => match assign.inner() {
                ast::Assignment::Single { target, op, expr } => {
                    self.visit_ast_expr(expr.inner(), shadowed);
                    self.visit_ast_target(
                        target.inner(),
                        *op == ast::AssignmentOp::Merge,
                        shadowed,
                    );
                }
                ast::Assignment::Infallible { ok, err, op, expr } => {
                    self.visit_ast_expr(expr.inner(), shadowed);
                    let is_merge = *op == ast::AssignmentOp::Merge;
                    self.visit_ast_target(ok.inner(), is_merge, shadowed);
                    self.visit_ast_target(err.inner(), is_merge, shadowed);
                }
            },
            ast::Expr::Query(query) => match query.inner().target.inner() {
                ast::QueryTarget::Internal(ident) => self.record_read_local(ident, shadowed),
                ast::QueryTarget::External(PathPrefix::Event) => self.event.mark_read(),
                ast::QueryTarget::External(PathPrefix::Metadata) => self.metadata.mark_read(),
                ast::QueryTarget::Container(c) => self.visit_ast_container(c, shadowed),
                ast::QueryTarget::FunctionCall(fc) => self.visit_ast_function_call(fc, shadowed),
            },
            ast::Expr::FunctionCall(fc) => self.visit_ast_function_call(fc.inner(), shadowed),
            ast::Expr::Unary(u) => match u.inner() {
                ast::Unary::Not(not) => self.visit_ast_expr(not.inner().1.inner(), shadowed),
            },
            ast::Expr::Abort(abort) => {
                if let Some(msg) = &abort.inner().message {
                    self.visit_ast_expr(msg.inner(), shadowed);
                }
            }
            ast::Expr::Return(ret) => self.visit_ast_expr(ret.inner().expr.inner(), shadowed),
        }
    }
}

pub(crate) const WIDEN_AFTER_ITERATIONS: usize = 4;
pub(crate) const FALLBACK_WIDEN_ITERATIONS: usize = 64;

fn canonicalize_kind(kind: &mut Kind) {
    if let Some(arr) = kind.as_array_mut() {
        for child in arr.known_mut().values_mut() {
            canonicalize_kind(child);
        }
    }
    if let Some(obj) = kind.as_object_mut() {
        for child in obj.known_mut().values_mut() {
            canonicalize_kind(child);
        }
    }
    *kind = kind.canonicalize();
}

pub(crate) fn canonicalize_type_state(state: &mut TypeState) {
    for details in state.local.bindings.values_mut() {
        canonicalize_kind(details.type_def.kind_mut());
    }

    let mut target_details = state.external.target().clone();
    canonicalize_kind(target_details.type_def.kind_mut());
    state.external.update_target(target_details);

    let mut canonical_metadata = state.external.metadata_kind().clone();
    canonicalize_kind(&mut canonical_metadata);
    state.external.update_metadata(canonical_metadata);
}

pub(crate) fn widen_differing_states(prev: &TypeState, next: &mut TypeState, force_top: bool) {
    for (ident, next_details) in &mut next.local.bindings {
        if let Some(prev_details) = prev.local.bindings.get(ident)
            && prev_details != next_details
        {
            if force_top {
                *next_details.type_def.kind_mut() = Kind::any();
                next_details.value = None;
            } else if prev_details.type_def.kind() != next_details.type_def.kind() {
                widen_differing_collections(
                    prev_details.type_def.kind(),
                    next_details.type_def.kind_mut(),
                    0,
                );
            }
        }
    }

    if prev.external.target() != next.external.target() {
        let mut target_details = next.external.target().clone();
        if force_top {
            *target_details.type_def.kind_mut() = Kind::any();
            target_details.value = None;
        } else if prev.external.target_kind() != next.external.target_kind() {
            widen_differing_collections(
                prev.external.target_kind(),
                target_details.type_def.kind_mut(),
                0,
            );
        }
        next.external.update_target(target_details);
    }

    if prev.external.metadata_kind() != next.external.metadata_kind() {
        if force_top {
            next.external.update_metadata(Kind::any());
        } else {
            let mut metadata_kind = next.external.metadata_kind().clone();
            widen_differing_collections(prev.external.metadata_kind(), &mut metadata_kind, 0);
            next.external.update_metadata(metadata_kind);
        }
    }

    canonicalize_type_state(next);
}

fn has_structural_collection_change(prev: &Kind, next: &Kind) -> bool {
    if (next.contains_array() && !prev.contains_array())
        || (next.contains_object() && !prev.contains_object())
    {
        return true;
    }

    if let (Some(prev_arr), Some(next_arr)) = (prev.as_array(), next.as_array())
        && prev_arr != next_arr
    {
        let same_keys = prev_arr.known().len() == next_arr.known().len()
            && prev_arr.unknown_kind() == next_arr.unknown_kind()
            && next_arr
                .known()
                .keys()
                .all(|k| prev_arr.known().contains_key(k));
        if !same_keys {
            return true;
        }
        for (k, next_child) in next_arr.known() {
            if let Some(prev_child) = prev_arr.known().get(k)
                && prev_child != next_child
                && has_structural_collection_change(prev_child, next_child)
            {
                return true;
            }
        }
    }

    if let (Some(prev_obj), Some(next_obj)) = (prev.as_object(), next.as_object())
        && prev_obj != next_obj
    {
        let same_keys = prev_obj.known().len() == next_obj.known().len()
            && prev_obj.unknown_kind() == next_obj.unknown_kind()
            && next_obj
                .known()
                .keys()
                .all(|k| prev_obj.known().contains_key(k));
        if !same_keys {
            return true;
        }
        for (k, next_child) in next_obj.known() {
            if let Some(prev_child) = prev_obj.known().get(k)
                && prev_child != next_child
                && has_structural_collection_change(prev_child, next_child)
            {
                return true;
            }
        }
    }

    false
}

const MAX_OBJECT_WIDEN_DEPTH: usize = 8;

fn widen_differing_collections(prev: &Kind, next: &mut Kind, depth: usize) {
    if !has_structural_collection_change(prev, next) {
        return;
    }

    if let Some(next_arr) = next.as_array_mut()
        && prev.as_array() != Some(next_arr)
    {
        *next_arr = Collection::any();
    }

    let mut recurred_object = false;
    if depth < MAX_OBJECT_WIDEN_DEPTH
        && let Some(prev_obj) = prev.as_object()
        && let Some(next_obj) = next.as_object_mut()
        && prev_obj != next_obj
    {
        let same_keys = prev_obj.known().len() == next_obj.known().len()
            && prev_obj.unknown_kind() == next_obj.unknown_kind()
            && next_obj
                .known()
                .keys()
                .all(|k| prev_obj.known().contains_key(k));
        if same_keys {
            recurred_object = true;
            for (k, next_child) in next_obj.known_mut() {
                if let Some(prev_child) = prev_obj.known().get(k)
                    && prev_child != next_child
                {
                    widen_differing_collections(prev_child, next_child, depth + 1);
                }
            }
        }
    }

    if !recurred_object
        && let Some(next_obj) = next.as_object_mut()
        && prev.as_object() != Some(next_obj)
    {
        *next_obj = Collection::any();
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
                let val_kind = if iterable_kind.is_never() {
                    Kind::never()
                } else {
                    iterable_kind
                        .as_array()
                        .map_or_else(Kind::any, Collection::reduced_kind)
                        .without_undefined()
                };
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

            let (key_kind, val_kind) = if iterable_kind.is_never() {
                (Kind::never(), Kind::never())
            } else {
                let has_array = iterable_kind.contains_array();
                let has_object = iterable_kind.contains_object();
                let arr_val = iterable_kind
                    .as_array()
                    .map_or_else(Kind::any, Collection::reduced_kind)
                    .without_undefined();
                let obj_val = iterable_kind
                    .as_object()
                    .map_or_else(Kind::any, Collection::reduced_kind)
                    .without_undefined();

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
        let mut guard = BindingScopeGuard::new(&mut ctx, Some(&x_id), Some(&y_id));

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
            let mut guard = BindingScopeGuard::new(&mut ctx, Some(&x_id), None);
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
            let mut guard = BindingScopeGuard::new(&mut ctx, Some(&x_id), None);
            guard.set_single(Value::from("in_loop"));
            assert_eq!(
                guard.ctx_mut().state().variable(&x_id),
                Some(&Value::from("in_loop"))
            );
        }

        assert!(state.variable(&x_id).is_none());
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
    fn test_scope_guard_selective_setters() {
        let mut target = Value::Null;
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let k_id = Ident::new("k");
        let v_id = Ident::new("v");

        // 1. Both present
        {
            let mut guard = BindingScopeGuard::new(&mut ctx, Some(&k_id), Some(&v_id));
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
        assert!(ctx.state().variable(&k_id).is_none());
        assert!(ctx.state().variable(&v_id).is_none());

        // 2. Only first present (second wildcard)
        {
            let mut guard_first = BindingScopeGuard::new(&mut ctx, Some(&k_id), None);
            guard_first.set_first(Value::from("key2"));
            guard_first.set_second(Value::Integer(200)); // no-op for second
            assert_eq!(
                guard_first.ctx_mut().state().variable(&k_id),
                Some(&Value::from("key2"))
            );
            assert!(guard_first.ctx_mut().state().variable(&v_id).is_none());
        }
        assert!(ctx.state().variable(&k_id).is_none());

        // 3. Only second present (first wildcard)
        {
            let mut guard_second = BindingScopeGuard::new(&mut ctx, None, Some(&v_id));
            guard_second.set_first(Value::from("key3")); // no-op for first
            guard_second.set_second(Value::Integer(300));
            assert!(guard_second.ctx_mut().state().variable(&k_id).is_none());
            assert_eq!(
                guard_second.ctx_mut().state().variable(&v_id),
                Some(&Value::Integer(300))
            );
        }
        assert!(ctx.state().variable(&v_id).is_none());

        // 4. Neither present (both wildcards)
        {
            let mut guard_none = BindingScopeGuard::new(&mut ctx, None, None);
            guard_none.set_first(Value::from("key4")); // no-op
            guard_none.set_second(Value::Integer(400)); // no-op
            assert!(guard_none.ctx_mut().state().variable(&k_id).is_none());
            assert!(guard_none.ctx_mut().state().variable(&v_id).is_none());
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
    fn test_for_type_info_propagates_fallibility() {
        let span = Span::new(10, 20);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let fallible_op = crate::compiler::expression::Op::new(
            Node::new(span, Expr::from(Value::from(1))),
            Node::new(span, crate::parser::ast::Opcode::Div),
            Node::new(span, Expr::from(Value::from(0))),
            &TypeState::default(),
        )
        .expect("valid op");
        let iterable = Box::new(Expr::Op(fallible_op));
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

    #[test]
    fn test_for_type_info_fixed_point_widening() {
        // x mutates from integer to string. On the second iteration, x + 1 is fallible.
        // Because the compiler re-compiles the loop body against the fixed-point TypeState,
        // `y = x + 1` is caught as a fallible assignment (E103), and a bare `x + 1`
        // is caught as an unhandled fallible expression (E100).
        let assign_source = r#"
            x = 1
            for _ in [1, 2] {
                y = x + 1
                x = "s"
            }
        "#;
        let Err(err) = crate::compiler::compile(assign_source, &[]) else {
            panic!("expected fallible assignment error from mutating outer variable");
        };
        assert_eq!(err[0].code, codes::ExprCode::FallibleAssignment as usize);

        let expr_source = r#"
            x = 1
            for _ in [1, 2] {
                x + 1
                x = "s"
            }
        "#;
        let Err(err) = crate::compiler::compile(expr_source, &[]) else {
            panic!("expected fallible expression error from mutating outer variable");
        };
        assert_eq!(err[0].code, codes::ExprCode::FallibleExpression as usize);
    }

    #[test]
    fn test_for_type_info_propagates_iterable_returns() {
        let source = r#"
            for _ in { if .x == 1 { return "foo" }; [1, 2] } {
                return 1
            }
        "#;
        let Ok(info) = crate::compiler::compile(source, &[]) else {
            panic!("expected successful compilation");
        };
        let final_type_info = info.program.final_type_info();
        let returns = final_type_info.result.returns();
        assert!(returns.is_superset(&crate::value::Kind::integer()).is_ok());
        assert!(returns.is_superset(&crate::value::Kind::bytes()).is_ok());
    }

    #[test]
    fn test_for_array_element_divergence_preserves_returns() {
        let source = r#"
            for _ in [1] {
                [{ if true { return "early" }; 1 }, { break; 2 }]
            }
        "#;
        let info = crate::compiler::compile(source, &[]).expect("compiles");
        let returns = info.program.final_type_info().result.returns().clone();
        assert!(
            returns.is_superset(&crate::value::Kind::bytes()).is_ok(),
            "expected early array divergence to preserve return kind bytes, got: {returns}"
        );
    }

    #[test]
    fn test_for_type_info_recursive_type_growth_terminates() {
        // 1. Recursive local array growth terminates and preserves integer | array
        let res_arr = crate::compiler::compile("x = 1\nfor _ in [1, 2] { x = [x] }\nx", &[]);
        assert!(res_arr.is_ok());
        let kind_arr = res_arr
            .unwrap()
            .program
            .final_type_info()
            .result
            .kind()
            .clone();
        assert!(kind_arr.contains_integer());
        assert!(kind_arr.contains_array());
        assert!(!kind_arr.contains_bytes());

        // 2. Recursive local object growth terminates and preserves integer | object
        let res_obj =
            crate::compiler::compile("x = 1\nfor _ in [1, 2] { x = { \"a\": x } }\nx", &[]);
        assert!(res_obj.is_ok());
        let kind_obj = res_obj
            .unwrap()
            .program
            .final_type_info()
            .result
            .kind()
            .clone();
        assert!(kind_obj.contains_integer());
        assert!(kind_obj.contains_object());
        assert!(!kind_obj.contains_bytes());

        // 3. Recursive external target growth terminates while preserving root object and sibling fields
        let res_ext = crate::compiler::compile(
            ".host = \"web-01\"\n.x = 1\nfor _ in [1, 2] { .x = [.x] }\n. |= { \"env\": \"prod\" }\n.host",
            &[],
        );
        assert!(res_ext.is_ok());
        let kind_host = res_ext
            .unwrap()
            .program
            .final_type_info()
            .result
            .kind()
            .clone();
        assert!(kind_host.is_bytes());

        // 4. Recursive array starting as array stays purely an array (can be iterated in a follow-up for loop)
        let res_iter = crate::compiler::compile(
            "x = [1]\nfor _ in [1, 2] { x = [x] }\nfor item in x { .last = item }",
            &[],
        );
        assert!(res_iter.is_ok());
    }

    #[test]
    fn test_compile_for_multi_iteration_type_state() {
        // 1. Post-loop state reflects multi-iteration fixed-point widening (r4091390581)
        let Err(err_post_loop) = crate::compiler::compile(
            "a = 1\nb = 1\nfor _ in [1, 2] {\n    b = a\n    a = \"str\"\n}\nc = b + 1",
            &[],
        ) else {
            panic!("expected c = b + 1 after loop to fail as fallible assignment");
        };
        assert_eq!(
            err_post_loop[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 2. In-loop error coalescing `??` is allowed when variable becomes fallible on iteration 2 (r4090101149)
        let res_coalesce = crate::compiler::compile(
            "x = 1\nfor _ in [1, 2] {\n    .y = (x + 1) ?? 0\n    x = \"str\"\n}",
            &[],
        );
        assert!(
            res_coalesce.is_ok(),
            "expected (x + 1) ?? 0 to compile when x widens to int|str, got: {:?}",
            res_coalesce.err()
        );

        // 3. In-loop infallible assignment `ok, err =` is allowed when variable becomes fallible on iteration 2
        let res_infallible_assign = crate::compiler::compile(
            "x = 1\nfor _ in [1, 2] {\n    .y, .err = x + 1\n    x = \"str\"\n}",
            &[],
        );
        assert!(
            res_infallible_assign.is_ok(),
            "expected .y, .err = x + 1 to compile when x widens to int|str, got: {:?}",
            res_infallible_assign.err()
        );

        // 4. In-loop merge `|=` rejects variable that becomes non-object on iteration 2 (E660)
        let Err(err_merge) = crate::compiler::compile(
            "x = {}\nfor _ in [1, 2] {\n    x |= { \"a\": 1 }\n    x = 42\n}",
            &[],
        ) else {
            panic!("expected x |= {{ \"a\": 1 }} to fail when x becomes integer on iteration 2");
        };
        assert_eq!(
            err_merge[0].code,
            codes::CompilerCode::MergeNonObjects as usize
        );

        // 5. In-loop nested `for` rejects iterable that becomes non-collection on iteration 2 (E633)
        let Err(err_nested_for) = crate::compiler::compile(
            "arr = [1]\nfor _ in [1, 2] {\n    for item in arr { .y = item }\n    arr = 42\n}",
            &[],
        ) else {
            panic!("expected inner for loop to fail when arr becomes integer on iteration 2");
        };
        assert_eq!(
            err_nested_for[0].code,
            codes::CompilerCode::NonCollectionIterable as usize
        );

        // 6. Nested loop coalescing succeeds when outer loop widens variable after inner loop
        let res_nested_coalesce = crate::compiler::compile(
            "x = 1\nfor _ in [1, 2] {\n    for _ in [1, 2] {\n        .y = (x + 1) ?? 0\n    }\n    x = \"str\"\n}",
            &[],
        );
        assert!(res_nested_coalesce.is_ok());

        // 7. Genuinely unnecessary coalesce inside a loop (when variable never widens) still fails (E651)
        let Err(err_unnecessary_coalesce) =
            crate::compiler::compile("x = 1\nfor _ in [1, 2] {\n    .y = (x + 1) ?? 0\n}", &[])
        else {
            panic!("expected unnecessary coalesce to fail when x never becomes fallible");
        };
        assert_eq!(
            err_unnecessary_coalesce[0].code,
            codes::CompilerCode::UnnecessaryCoalesce as usize
        );
    }

    #[test]
    fn test_in_as_contextual_keyword_identifier() {
        let tz = TimeZone::default();
        let eval = |script: &str| -> (Resolved, RuntimeState) {
            let prog = crate::compiler::compile(script, &[]).unwrap();
            let mut target = TargetValue::new(Value::Null);
            let mut state = RuntimeState::default();
            let mut ctx = Context::new(&mut target, &mut state, &tz);
            let res = prog.program.resolve(&mut ctx);
            (res, state)
        };

        // 1. Local variable assignment and arithmetic expression (`in = 1; in + 2`)
        let (res1, _) = eval("in = 1\nin + 2");
        assert_eq!(res1.unwrap(), Value::Integer(3));

        // 2. Internal query target (`in = { "foo": "bar" }; in.foo`)
        let (res2, _) = eval("in = { \"foo\": \"bar\" }\nin.foo");
        assert_eq!(res2.unwrap(), Value::from("bar"));

        // 3. Loop variable named `in` (`for in in [1, 2] { ... }`)
        let (res3, _) = eval("acc = 0\nfor in in [1, 2, 3] { acc = acc + in }\nacc");
        assert_eq!(res3.unwrap(), Value::Integer(6));

        // 4. Iterable variable named `in` (`for x in in { ... }`)
        let (res4, _) = eval("in = [10, 20]\nacc = 0\nfor x in in { acc = acc + x }\nacc");
        assert_eq!(res4.unwrap(), Value::Integer(30));

        // 5. Infallible assignment target named `in`
        let (res5, _) = eval("d = if true { 1 } else { 1 }\nin, err = 2 / d\nin");
        assert_eq!(
            res5.unwrap(),
            Value::Float(ordered_float::NotNan::new(2.0).unwrap())
        );
    }

    #[test]
    fn test_for_type_info_long_shift_chain_reaches_fixed_point() {
        // 1. 10-variable shift chain (> 8 iterations) must not exit early before reaching fixed point (r4328991311)
        let source = r#"
            x0 = 1; x1 = 1; x2 = 1; x3 = 1; x4 = 1; x5 = 1; x6 = 1; x7 = 1; x8 = 1; x9 = 1
            for _ in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                x9 = x8
                x8 = x7
                x7 = x6
                x6 = x5
                x5 = x4
                x4 = x3
                x3 = x2
                x2 = x1
                x1 = x0
                x0 = "str"
            }
            .out = x9 + 1
        "#;
        let Err(err) = crate::compiler::compile(source, &[]) else {
            panic!(
                "expected .out = x9 + 1 to fail as fallible assignment after 10-step propagation"
            );
        };
        assert_eq!(err[0].code, codes::ExprCode::FallibleAssignment as usize);

        // 2. Shift chain across object fields converges without destructively widening sibling fields
        let obj_shift_source = r#"
            s = { "x0": 1, "x1": 1, "x2": 1, "x3": 1, "x4": 1, "x5": 1, "keep": 42 }
            for _ in [1, 2, 3, 4, 5, 6] {
                s.x5 = s.x4
                s.x4 = s.x3
                s.x3 = s.x2
                s.x2 = s.x1
                s.x1 = s.x0
                s.x0 = "str"
            }
            .ok = s.keep + 1
            .fail = s.x5 + 1
        "#;
        let Err(err_obj) = crate::compiler::compile(obj_shift_source, &[]) else {
            panic!("expected .fail = s.x5 + 1 to fail as fallible assignment");
        };
        assert_eq!(err_obj.len(), 1);
        assert_eq!(
            err_obj[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 3. Deeply nested collection widening + delayed scalar shift preserves sibling fields at depth >= 2
        let deep_nested_source = r#"
            s = {
                "a": {
                    "b": {
                        "arr": [1],
                        "keep": 42
                    }
                },
                "x0": [1], "x1": [1], "x2": [1], "x3": [1], "x4": [1]
            }
            for _ in [1, 2, 3, 4, 5, 6] {
                s.a.b.arr = if true { [s.a.b.arr] } else { s.x4 }
                s.x4 = s.x3
                s.x3 = s.x2
                s.x2 = s.x1
                s.x1 = s.x0
                s.x0 = "str"
            }
            .ok = s.a.b.keep + 1
        "#;
        let res_deep = crate::compiler::compile(deep_nested_source, &[]);
        assert!(
            res_deep.is_ok(),
            "expected s.a.b.keep to remain integer after nested array widening + delayed scalar shift, got: {:?}",
            res_deep.err()
        );
    }

    #[test]
    fn test_for_stateful_iterable_type_info_evaluated_once() {
        // Stateful iterable `{ x = x[0]; x }` must be evaluated against pre-iterable state (r4328991318)
        let source = r"
            x = [[1, 2]]
            for y in { x = x[0]; x } {
                .last = y + 1
            }
            for z in x {
                .z = z + 1
            }
        ";
        let res = crate::compiler::compile(source, &[]);
        assert!(
            res.is_ok(),
            "expected stateful iterable to compile cleanly, got: {:?}",
            res.err()
        );
    }

    #[test]
    fn test_for_multi_pass_infallible_assignment_default_widening() {
        // Pass 1 sees `x: array` (exact) so `y, err = ...` gets `default = []` (array).
        // Pass 2 sees `x: array | object` (non-exact) so `y, err = ...` gets `default = null`.
        // Pass 3 must observe that `y` is now `array | object | null` and reject `for _, item in y` (E633).
        let source = r#"
            x = [1]
            y = [1]
            d = 1
            for _ in [1, 2, 3] {
                for _, item in y {
                    .last = item
                }
                y, err = { 1 / d; x }
                x = { "a": 1 }
                d = 0
            }
        "#;
        let Err(err) = crate::compiler::compile(source, &[]) else {
            panic!(
                "expected inner for loop over `y` to fail once `y`'s error-default widens to null"
            );
        };
        assert_eq!(
            err[0].code,
            codes::CompilerCode::NonCollectionIterable as usize
        );
    }

    #[test]
    fn test_for_closure_preserves_speculative_coalesce() {
        let fns = crate::stdlib::all();

        // 1. Closure inside a loop must participate in speculative pass when outer variable widens later (r4343753072)
        let source_widens = r#"
            x = 1
            for _ in [1, 2] {
                for_each([1]) -> |_i, _v| {
                    .y = (x + 1) ?? 0
                }
                x = "s"
            }
        "#;
        let res = crate::compiler::compile(source_widens, &fns);
        assert!(
            res.is_ok(),
            "expected closure inside loop to allow speculative coalesce when x widens, got: {:?}",
            res.err()
        );

        // 2. Closure inside a loop must still reject unnecessary coalesce when variable never widens
        let source_infallible = r"
            x = 1
            for _ in [1, 2] {
                for_each([1]) -> |_i, _v| {
                    .y = (x + 1) ?? 0
                }
            }
        ";
        let Err(err) = crate::compiler::compile(source_infallible, &fns) else {
            panic!("expected unnecessary coalesce inside closure to fail when x never widens");
        };
        assert_eq!(
            err[0].code,
            codes::CompilerCode::UnnecessaryCoalesce as usize
        );
    }

    #[test]
    fn test_for_break_excluded_from_back_edge() {
        // 1. Unconditional break exits on first iteration; break state must not feed back into next iteration (r4343753074)
        let unconditional_break = r"
            x = [0, 1]
            for _ in [1, 2] {
                x = [x, 1]
                break
            }
            .y = x[1] + 1
        ";
        let res_uncond = crate::compiler::compile(unconditional_break, &[]);
        assert!(
            res_uncond.is_ok(),
            "expected unconditional break not to widen x across back edge, got: {:?}",
            res_uncond.err()
        );

        // 2. Conditional break branch must not feed back into next iteration
        let conditional_break = r"
            x = [0, 1]
            for i, _ in [1, 2] {
                if i == 1 {
                    x = [x, 1]
                    break
                }
            }
            .y = x[1] + 1
        ";
        let res_cond = crate::compiler::compile(conditional_break, &[]);
        assert!(
            res_cond.is_ok(),
            "expected conditional break not to widen x across back edge, got: {:?}",
            res_cond.err()
        );

        // 3. Break branch mutations still flow to post-loop state
        let break_flows_post_loop = r#"
            x = 1
            for i, _ in [1, 2] {
                if i == 1 {
                    x = "broken"
                    break
                }
            }
            .y = x + 1
        "#;
        let Err(err_post) = crate::compiler::compile(break_flows_post_loop, &[]) else {
            panic!("expected break mutation to be visible after the loop");
        };
        assert_eq!(
            err_post[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 4. Continue branch mutations DO flow along the back edge to subsequent iterations
        let continue_flows_back_edge = r#"
            x = 1
            for i, _ in [1, 2] {
                .y = x + 1
                if i == 0 {
                    x = "continued"
                    continue
                }
            }
        "#;
        let Err(err_cont) = crate::compiler::compile(continue_flows_back_edge, &[]) else {
            panic!("expected continue mutation to flow along the back edge");
        };
        assert_eq!(
            err_cont[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 5. Conditional break inside a binary operator does not leak onto the back edge
        let nested_expr_break = r"
            x = [0, 1]
            for i, _ in [1, 2] {
                .tmp2 = ({
                    if i == 1 {
                        x = [x, 1]
                        break
                    }
                    1
                }) + 2
            }
            .y = x[1] + 1
        ";
        let res_nested = crate::compiler::compile(nested_expr_break, &[]);
        assert!(
            res_nested.is_ok(),
            "expected conditional break inside op not to widen x across back edge, got: {:?}",
            res_nested.err()
        );
    }

    #[test]
    fn test_for_speculative_passes_rollback_compile_config() {
        use crate::compiler::{
            CompileConfig, Function,
            function::{ArgumentList, Compiled, FunctionCompileContext},
        };

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct CompileCounter(usize);

        #[derive(Clone, Copy, Debug)]
        struct CountCompileFn;

        #[derive(Clone, Debug, PartialEq)]
        struct NoopExpr;

        impl Expression for NoopExpr {
            fn resolve(&self, _ctx: &mut Context) -> Resolved {
                Ok(Value::Null)
            }

            fn type_info(&self, state: &TypeState) -> TypeInfo {
                TypeInfo::new(state, TypeDef::null())
            }
        }

        impl Function for CountCompileFn {
            fn identifier(&self) -> &'static str {
                "count_compile"
            }

            fn description(&self) -> &'static str {
                "count_compile"
            }

            fn category(&self) -> &'static str {
                "test"
            }

            fn return_kind(&self) -> u16 {
                crate::compiler::value::kind::NULL
            }

            fn examples(&self) -> &'static [crate::compiler::function::Example] {
                &[]
            }

            fn compile(
                &self,
                _state: &TypeState,
                ctx: &mut FunctionCompileContext,
                _arguments: ArgumentList,
            ) -> Compiled {
                if let Some(counter) = ctx.get_external_context_mut::<CompileCounter>() {
                    counter.0 += 1;
                }
                Ok(Box::new(NoopExpr))
            }
        }

        let mut config = CompileConfig::default();
        config.set_custom(CompileCounter(0));
        let fns: Vec<Box<dyn Function>> = vec![Box::new(CountCompileFn)];

        let source = r#"
            x = 1
            for _ in [1, 2] {
                count_compile()
                x = "s"
            }
        "#;
        let program = crate::parser::parse(source).expect("parses");
        let res = crate::compiler::Compiler::compile(&fns, program, &TypeState::default(), config)
            .expect("compiles");

        assert_eq!(
            res.config.get_custom::<CompileCounter>(),
            Some(&CompileCounter(1)),
            "expected CompileConfig custom state from speculative pass to be rolled back"
        );
    }

    #[test]
    fn test_for_nested_loop_iterable_outer_jumps() {
        // 1. Conditional break inside nested loop's iterable exits the enclosing loop
        let outer_break_in_nested_iterable = r#"
            x = 1
            for _ in [1] {
                for _ in { if true { x = "s"; break }; x = 1; [1] } {
                    null
                }
            }
            .y = x + 1
        "#;
        let Err(err_break) = crate::compiler::compile(outer_break_in_nested_iterable, &[]) else {
            panic!("expected break in nested iterable to propagate x = \"s\" to outer loop exit");
        };
        assert_eq!(
            err_break[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 2. Conditional continue inside nested loop's iterable continues the enclosing loop
        let outer_continue_in_nested_iterable = r#"
            x = 1
            for _ in [1, 2] {
                .y = x + 1
                for _ in { if true { x = "s"; continue }; x = 1; [1] } {
                    null
                }
            }
        "#;
        let Err(err_cont) = crate::compiler::compile(outer_continue_in_nested_iterable, &[]) else {
            panic!(
                "expected continue in nested iterable to propagate x = \"s\" along outer back-edge"
            );
        };
        assert_eq!(
            err_cont[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 3. Unconditional break in nested iterable skips statements after the nested loop
        let unconditional_break_in_nested_iterable = r#"
            x = 1
            for _ in [1] {
                for _ in { x = "s"; break; [1] } {
                    null
                }
                x = 1
            }
            .y = x + 1
        "#;
        let Err(err_uncond) = crate::compiler::compile(unconditional_break_in_nested_iterable, &[])
        else {
            panic!(
                "expected unconditional break in nested iterable to skip trailing x = 1 in outer loop"
            );
        };
        assert_eq!(
            err_uncond[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 4. Conditional break inside nested iterable where the reset `x = 1` follows the nested loop
        let break_in_iterable_before_outer_reset = r#"
            x = 1
            for _ in [1] {
                for _ in { if true { x = "s"; break }; [1] } {
                    null
                }
                x = 1
            }
            .y = x + 1
        "#;
        let Err(err_post_reset) =
            crate::compiler::compile(break_in_iterable_before_outer_reset, &[])
        else {
            panic!("expected break in nested iterable to skip trailing x = 1 in outer loop");
        };
        assert_eq!(
            err_post_reset[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 5. Jumps in nested loop body are captured by the nested loop (falling through to `x = 1`),
        // while outer iterable jump is pruned from statements following the nested loop in the outer loop
        let mixed_nested = r#"
            x = 1
            z = 1
            for i, _ in [1, 2] {
                for _ in { if i == 1 { z = "outer_exit"; break }; [1, 2] } {
                    if i == 0 {
                        x = "inner_break_only"
                        break
                    }
                }
                x = 1
                .in_outer = z + 1
            }
            .after_x = x + 1
        "#;
        let res_mixed = crate::compiler::compile(mixed_nested, &[]);
        assert!(
            res_mixed.is_ok(),
            "expected inner body break to be captured by inner loop and outer iterable break to be pruned, got: {:?}",
            res_mixed.err()
        );
    }

    #[test]
    fn test_for_function_argument_loop_jump_boundary() {
        let fns = crate::stdlib::all();

        // 1. `break` inside a function argument cannot target an enclosing `for` loop
        // (prevents unsoundness with functions like `decode_punycode!` that skip arguments)
        let skipped_arg_break = r#"
            x = "a"
            for _ in [1] {
                decode_punycode!("plain", validate: { break; true })
                x = 1
            }
            snakecase(x)
        "#;
        let Err(err1) = crate::compiler::compile(skipped_arg_break, &fns) else {
            panic!("expected break in function argument to be rejected");
        };
        assert_eq!(err1[0].code, codes::CompilerCode::BreakOutsideLoop as usize);

        // 2. `continue` inside a function argument cannot target an enclosing `for` loop
        let arg_continue = r#"
            for _ in [1, 2] {
                .tmp = contains(substring: { if true { continue }; "a" }, value: "a")
            }
        "#;
        let Err(err2) = crate::compiler::compile(arg_continue, &fns) else {
            panic!("expected continue in function argument to be rejected");
        };
        assert_eq!(err2[0].code, codes::CompilerCode::BreakOutsideLoop as usize);

        // 3. `for` nested inside `for_each` does not leak `for_each`'s breakable context into
        // function arguments inside the `for` loop
        let for_in_for_each_arg_break = r"
            for_each([1, 2]) -> |_i, _v| {
                for _ in [1] {
                    .tmp = to_string({ break; 1 })
                }
            }
        ";
        let Err(err3) = crate::compiler::compile(for_in_for_each_arg_break, &fns) else {
            panic!("expected break in function argument inside nested for loop to be rejected");
        };
        assert_eq!(err3[0].code, codes::CompilerCode::BreakOutsideLoop as usize);

        // 4. `for_each` after a nested `for` loop still allows `break` in its own closure
        let for_each_after_nested_for = r"
            count = 0
            for_each([1, 2, 3]) -> |_i, v| {
                for _ in [1] {
                    count = count + 1
                }
                if v == 2 {
                    break
                }
            }
            count
        ";
        let compiled_after =
            crate::compiler::compile(for_each_after_nested_for, &fns).expect("compiles");
        let mut target = TargetValue::new(Value::Null);
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_after.program.resolve(&mut ctx), Ok(Value::from(2)));

        // 5. A `for` loop nested inside a function argument can still break/continue itself
        let nested_for_in_arg = r"
            outer = 0
            for _ in [1, 2] {
                .s = to_string({
                    sum = 0
                    for v in [1, 2, 3, 4] {
                        if v == 2 { continue }
                        if v == 4 { break }
                        sum = sum + v
                    }
                    sum
                })
                outer = outer + 1
            }
            [outer, .s]
        ";
        let compiled_nested = crate::compiler::compile(nested_for_in_arg, &fns).expect("compiles");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_nested.program.resolve(&mut ctx),
            Ok(Value::from(vec![Value::from(2), Value::from("4")]))
        );
    }

    #[test]
    fn test_for_excludes_undefined_from_merged_collections_and_allows_empty_nested_iterables() {
        // 1. Merged arrays of differing lengths do not leak `undefined` into bound loop values
        let merged_arrays_single = r"
            rows = if true { [[1, 2]] } else { [] }
            for row in rows {
                for v in row {
                    .last = v + 1
                }
            }
        ";
        assert!(
            crate::compiler::compile(merged_arrays_single, &[]).is_ok(),
            "expected merged-length array iteration to exclude undefined"
        );

        // 2. Key-value iteration over merged arrays and merged objects excludes `undefined`
        let merged_kv = r#"
            rows = if true { [[1]] } else { [] }
            for _, row in rows {
                for v in row {
                    .a = v + 1
                }
            }
            objs = if true { { "k": [1] } } else { {} }
            for _, row in objs {
                for v in row {
                    .b = v + 1
                }
            }
        "#;
        assert!(
            crate::compiler::compile(merged_kv, &[]).is_ok(),
            "expected key-value iteration over merged collections to exclude undefined"
        );

        // 3. Nested iteration over elements of statically empty collections (`Kind::never()`) compiles
        let empty_nested = r"
            for xs in [] {
                for x in xs {
                    for y in x {
                        .v = y
                    }
                }
            }
            for _, xs in {} {
                for x in xs {
                    .v = x
                }
            }
        ";
        assert!(
            crate::compiler::compile(empty_nested, &[]).is_ok(),
            "expected nested iteration over statically empty collection to compile"
        );
    }

    #[test]
    fn test_for_defers_invalid_argument_kind_until_loop_widening() {
        let fns = crate::stdlib::all();

        // 1. Loop-carried variable starts as `null` and widens to `null | string`:
        // `upcase(prev) ?? ""` must compile once the loop header widens.
        let deferred_valid = r#"
            prev = null
            for v in ["a", "b"] {
                if prev != null {
                    .y = upcase(prev) ?? ""
                }
                prev = v
            }
            .y
        "#;
        let compiled = crate::compiler::compile(deferred_valid, &fns)
            .expect("expected upcase(prev) ?? \"\" to compile after loop widening");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from("A")));

        // 2. Truly invalid argument type that never widens still reports ONLY E110 (no cascading E651).
        let truly_invalid = r#"
            prev = null
            for v in [1, 2] {
                .y = upcase(prev) ?? ""
                prev = v
            }
        "#;
        let Err(err) = crate::compiler::compile(truly_invalid, &fns) else {
            panic!("expected upcase(prev) with null | integer to fail with E110");
        };
        assert_eq!(err.len(), 1, "expected only E110 diagnostic, got: {err:?}");
        assert_eq!(err[0].code, codes::ExprCode::InvalidArgumentKind as usize);
    }

    #[test]
    fn test_for_deferred_call_preserves_return_type() {
        let fns = crate::stdlib::all();
        let tz = TimeZone::default();

        // 1. Deferred call inside a predicate (`starts_with(prev, "a") ?? false`) must preserve boolean return type
        let pred_source = r#"
            prev = null
            .hit = false
            for v in ["a", "b"] {
                if (starts_with(prev, "a") ?? false) {
                    .hit = true
                }
                prev = v
            }
            .hit
        "#;
        let compiled_pred = crate::compiler::compile(pred_source, &fns)
            .expect("expected deferred starts_with in predicate to compile after loop widening");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_pred.program.resolve(&mut ctx),
            Ok(Value::from(true))
        );

        // 2. Deferred call as a query target (`parse_json(prev).a ?? 0`)
        let query_source = r#"
            prev = null
            .val = 0
            for v in ["{\"a\": 1}", "{\"a\": 2}"] {
                if prev != null {
                    .val = parse_json(prev).a ?? 0
                }
                prev = v
            }
            .val
        "#;
        let compiled_query = crate::compiler::compile(query_source, &fns)
            .expect("expected deferred parse_json query target to compile after loop widening");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_query.program.resolve(&mut ctx), Ok(Value::from(1)));

        // 3. Truly invalid call in predicate that never widens still fails with E110 only
        let invalid_pred = r#"
            prev = null
            for v in [1, 2] {
                if (starts_with(prev, "a") ?? false) {
                    .hit = true
                }
                prev = v
            }
        "#;
        let Err(err) = crate::compiler::compile(invalid_pred, &fns) else {
            panic!("expected starts_with on null | integer to fail with E110");
        };
        assert_eq!(err.len(), 1, "expected only E110 diagnostic, got: {err:?}");
        assert_eq!(err[0].code, codes::ExprCode::InvalidArgumentKind as usize);
    }

    #[test]
    fn test_for_jump_aware_block_and_predicate_result_typing() {
        let tz = TimeZone::default();

        // 1. Block result after conditional `break` excludes the breaking branch's mutation
        let block_break = r#"
            x = 1
            for i, _ in [1, 2] {
                .y = {
                    if i == 1 {
                        x = "s"
                        break
                    }
                    x + 1
                }
            }
            .y
        "#;
        let compiled_block = crate::compiler::compile(block_break, &[])
            .expect("expected block after breaking branch to be infallible");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_block.program.resolve(&mut ctx), Ok(Value::from(2)));

        // 2. Contrast: conditional `continue` DOES feed `x = "s"` back into the next iteration
        let block_continue = r#"
            x = 1
            for i, _ in [1, 2] {
                .y = {
                    if i == 1 {
                        x = "s"
                        continue
                    }
                    x + 1
                }
            }
        "#;
        let Err(err_cont) = crate::compiler::compile(block_continue, &[]) else {
            panic!("expected continue mutation to make x + 1 fallible on iteration 2");
        };
        assert_eq!(
            err_cont[0].code,
            codes::ExprCode::FallibleAssignment as usize
        );

        // 3. Predicate block after conditional `break` stays boolean
        let pred_break = r#"
            x = true
            .hit = false
            for i, _ in [1, 2] {
                if { if i == 1 { x = "s"; break }; x } {
                    .hit = true
                }
            }
            .hit
        "#;
        let compiled_pred = crate::compiler::compile(pred_break, &[])
            .expect("expected predicate after breaking branch to stay boolean");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_pred.program.resolve(&mut ctx),
            Ok(Value::from(true))
        );

        // 4. Operator operand after conditional `break` stays infallible
        let op_break = r#"
            x = 1
            for i, _ in [1, 2] {
                .z = ({ if i == 1 { x = "s"; break }; x + 1 }) + 2
            }
            .z
        "#;
        let compiled_op = crate::compiler::compile(op_break, &[])
            .expect("expected op operand after breaking branch to stay infallible");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_op.program.resolve(&mut ctx), Ok(Value::from(4)));

        // 5. Infallible assignment (`ok, err =`) rejects infallible block after breaking branch (E104)
        let infallible_assign = r#"
            x = 1
            for i, _ in [1, 2] {
                .ok, .err = {
                    if i == 1 {
                        x = "s"
                        break
                    }
                    x + 1
                }
            }
        "#;
        let Err(err_infallible) = crate::compiler::compile(infallible_assign, &[]) else {
            panic!("expected infallible assignment on infallible block to fail with E104");
        };
        assert_eq!(
            err_infallible[0].code,
            codes::ExprCode::InfallibleAssignment as usize
        );
    }

    #[test]
    fn test_for_jump_aware_not_abort_and_return_validation() {
        let tz = TimeZone::default();

        // 1. `!{ ... }` excludes the breaking branch's mutation from the operand type check.
        let not_break = r#"
            b = true
            for i, _ in [1, 2] {
                .y = !{
                    if i == 1 {
                        b = "s"
                        break
                    }
                    b
                }
            }
            .y
        "#;
        let compiled_not = crate::compiler::compile(not_break, &[])
            .expect("expected negation of jump-aware boolean block to compile");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_not.program.resolve(&mut ctx),
            Ok(Value::from(false))
        );

        // 2. `abort { ... }` excludes the breaking branch's mutation from the message check.
        let abort_break = r#"
            msg = "err"
            for i, _ in [1, 2] {
                if i == 1 {
                    abort {
                        if i == 1 {
                            msg = 1
                            break
                        }
                        msg
                    }
                }
            }
            msg
        "#;
        let compiled_abort = crate::compiler::compile(abort_break, &[])
            .expect("expected abort with jump-aware string block to compile");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_abort.program.resolve(&mut ctx), Ok(Value::from(1)));

        // 3. `return { ... }` excludes the breaking branch's mutation from the fallibility check.
        let return_break = r#"
            x = 1
            for i, _ in [1, 2] {
                return {
                    if i == 0 {
                        x = "s"
                        break
                    }
                    x + 1
                }
            }
            x
        "#;
        let compiled_return = crate::compiler::compile(return_break, &[])
            .expect("expected return with jump-aware infallible block to compile");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_return.program.resolve(&mut ctx),
            Ok(Value::from("s"))
        );

        // 4. Negative: genuinely non-boolean negation still fails when the normal path mutates `b`.
        let invalid_not = r#"
            b = true
            for i, _ in [1, 2] {
                .y = !{
                    if i == 1 {
                        break
                    }
                    b = "s"
                    b
                }
            }
        "#;
        assert!(
            crate::compiler::compile(invalid_not, &[]).is_err(),
            "expected genuinely non-boolean negation to fail compilation"
        );
    }

    #[test]
    fn test_for_short_circuit_op_preserves_lhs_fallibility_and_returns() {
        // 1. Short-circuit `&&` on null LHS must preserve `return` types from LHS.
        let and_returns = r#"
            for i, _ in [1, 2] {
                .x = ({
                    if i == 1 {
                        break
                    }
                    if i == 0 {
                        return "hi"
                    }
                    null
                }) && true
            }
        "#;
        let compiled_and =
            crate::compiler::compile(and_returns, &[]).expect("short-circuit && compiles");
        assert_eq!(
            compiled_and.program.final_type_info().result.returns(),
            &Kind::bytes(),
            "expected short-circuit && to preserve string return type from LHS"
        );

        // 2. Short-circuit `||` on null LHS must preserve `return` types from LHS.
        let or_returns = r#"
            for i, _ in [1, 2] {
                .x = ({
                    if i == 1 {
                        break
                    }
                    if i == 0 {
                        return "hi"
                    }
                    null
                }) || 1
            }
        "#;
        let compiled_or =
            crate::compiler::compile(or_returns, &[]).expect("short-circuit || compiles");
        assert_eq!(
            compiled_or.program.final_type_info().result.returns(),
            &Kind::bytes(),
            "expected short-circuit || to preserve string return type from LHS"
        );

        // 3. Short-circuit `||` on null LHS must not swallow unhandled fallibility from LHS.
        let or_fallible_lhs = r#"
            for i, _ in [1, 2] {
                .x = ({
                    if i == 1 {
                        break
                    }
                    1 + "a"
                    null
                }) || 1
            }
        "#;
        assert!(
            crate::compiler::compile(or_fallible_lhs, &[]).is_err(),
            "expected short-circuit || to reject unhandled fallible LHS"
        );

        // 4. Short-circuit `&&` on null LHS must not swallow unhandled fallibility from LHS.
        let and_fallible_lhs = r#"
            for i, _ in [1, 2] {
                .x = ({
                    if i == 1 {
                        break
                    }
                    1 + "a"
                    null
                }) && true
            }
        "#;
        assert!(
            crate::compiler::compile(and_fallible_lhs, &[]).is_err(),
            "expected short-circuit && to reject unhandled fallible LHS"
        );
    }

    #[test]
    fn test_for_diverging_return_and_abort_excluded_from_back_edge() {
        let tz = TimeZone::default();

        // 1. Body ending in `return` cannot reach the back edge.
        let return_body = r#"
            x = 1
            for _ in [1, 2] {
                .y = x + 1
                x = "s"
                return 0
            }
        "#;
        let compiled_ret = crate::compiler::compile(return_body, &[])
            .expect("expected return-only loop body not to widen x across back edge");
        assert_eq!(
            compiled_ret.program.final_type_info().result.returns(),
            &Kind::integer()
        );
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert!(matches!(
            compiled_ret.program.resolve(&mut ctx),
            Err(ExpressionError::Return { value, .. }) if value == Value::from(0)
        ));
        assert_eq!(target.value.get(crate::path!("y")), Some(&Value::from(2)));

        // 2. Body ending in `abort` cannot reach the back edge.
        let abort_body = r#"
            x = 1
            for _ in [1, 2] {
                .y = x + 1
                x = "s"
                abort "stop"
            }
        "#;
        assert!(
            crate::compiler::compile(abort_body, &[]).is_ok(),
            "expected abort-only loop body not to widen x across back edge"
        );

        // 3. Conditional `return` branch does not leak its mutation to the normal path or back edge.
        let cond_return = r#"
            x = 1
            for i, _ in [1, 2] {
                if i == 1 {
                    x = "s"
                    return 0
                }
                .y = x + 1
            }
        "#;
        let compiled_cond = crate::compiler::compile(cond_return, &[])
            .expect("expected conditional return branch not to widen x");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert!(matches!(
            compiled_cond.program.resolve(&mut ctx),
            Err(ExpressionError::Return { value, .. }) if value == Value::from(0)
        ));
        assert_eq!(target.value.get(crate::path!("y")), Some(&Value::from(2)));

        // 4. Post-loop code after a diverging body keeps pre-loop type when iterable is empty.
        let post_loop = r#"
            x = 1
            for _ in [] {
                x = "s"
                return 0
            }
            x + 1
        "#;
        let compiled_post = crate::compiler::compile(post_loop, &[])
            .expect("expected post-loop code after diverging body to retain integer x");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_post.program.resolve(&mut ctx), Ok(Value::from(2)));
    }

    #[test]
    fn test_for_deeply_nested_loops_bound_compile_and_analysis() {
        use std::fmt::Write as _;

        let tz = TimeZone::default();
        let depth = 20;

        // 1. Codex's exact constant-churn pattern at depth 20:
        //    must compile quickly AND preserve exact integer typing (`.out = v19 + 1` stays infallible).
        let mut const_churn = String::new();
        for i in 0..depth {
            let _ = writeln!(const_churn, "v{i} = 0\nfor _ in [1] {{");
        }
        let _ = writeln!(const_churn, "v{} = 1\n.out = v{} + 1", depth - 1, depth - 1);
        for i in (0..depth - 1).rev() {
            let _ = writeln!(const_churn, "}}\nv{i} = 1");
        }
        const_churn.push_str("}\n.out\n");

        let compiled = crate::compiler::compile(&const_churn, &[])
            .expect("depth-20 constant-churn nested loops should compile cleanly");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from(2)));

        // 2. Independent type-widening nested loops at depth 20.
        let mut type_widen = String::new();
        for i in 0..depth {
            let _ = writeln!(type_widen, "v{i} = 1\nfor _ in [1] {{");
        }
        let _ = writeln!(type_widen, "v{} = \"s\"", depth - 1);
        for i in (0..depth - 1).rev() {
            let _ = writeln!(type_widen, "}}\nv{i} = \"s\"");
        }
        type_widen.push_str("}\nv0\n");

        let compiled_widen = crate::compiler::compile(&type_widen, &[])
            .expect("depth-20 independent type-widening nested loops should compile cleanly");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(
            compiled_widen.program.resolve(&mut ctx),
            Ok(Value::from("s"))
        );

        // 3. Adversarial dependent nested loops at depth 20 where the innermost body reads all outer vars.
        let mut dependent = String::new();
        for i in 0..depth {
            let _ = writeln!(dependent, "v{i} = 1\nfor _ in [1] {{");
        }
        let all_vars = (0..depth)
            .map(|i| format!("v{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(dependent, ".all = [{all_vars}]\nv{} = \"s\"", depth - 1);
        for i in (0..depth - 1).rev() {
            let _ = writeln!(dependent, "}}\nv{i} = \"s\"");
        }
        dependent.push_str("}\nv0\n");

        let compiled_dep = crate::compiler::compile(&dependent, &[])
            .expect("depth-20 dependent nested loops should compile in bounded time");
        let mut target = TargetValue::new(Value::Object(ObjectMap::new()));
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);
        assert_eq!(compiled_dep.program.resolve(&mut ctx), Ok(Value::from("s")));
    }

    #[test]
    fn test_for_budget_exhaustion_recomputes_return_types() {
        use std::fmt::Write as _;

        // A shift chain longer than `MAX_LOOP_REPASSES` exhausts the analysis budget
        // before `"s"` reaches the final variable `v_last`. After budget-triggered
        // widening, the loop body must be re-evaluated once so `returns()` includes `bytes`.
        let chain_len = MAX_LOOP_REPASSES + 8;
        let mut source = String::new();
        for i in 0..chain_len {
            let _ = writeln!(source, "v{i} = 0");
        }
        source.push_str("for _ in [1, 2] {\n");
        let _ = writeln!(
            source,
            "    if .flag == true {{ return v{} }}",
            chain_len - 1
        );
        for i in (1..chain_len).rev() {
            let _ = writeln!(source, "    v{i} = v{}", i - 1);
        }
        source.push_str("    v0 = \"s\"\n}\n");

        let compiled =
            crate::compiler::compile(&source, &[]).expect("long shift chain should compile");
        let returns = compiled.program.final_type_info().result.returns().clone();
        assert!(
            returns.is_superset(&Kind::bytes()).is_ok(),
            "expected budget-widened return type to include bytes, got: {returns}"
        );
        assert!(
            returns.is_superset(&Kind::integer()).is_ok(),
            "expected budget-widened return type to include integer, got: {returns}"
        );
    }
}
