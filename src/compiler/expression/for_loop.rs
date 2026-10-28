use std::fmt;

use crate::compiler::expression::{Block, Expr, Resolved};
use crate::compiler::parser::ast::{ForPattern, Ident};
use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{Context, Expression, ExpressionError, Span, TypeDef, codes};
use crate::diagnostic::{DiagnosticMessage, Label};
use crate::value::kind::Collection;
use crate::value::{Kind, ObjectMap, Value};

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

impl For {
    pub(crate) fn eval_for_in_loop(&self, state: &TypeState) -> (TypeState, LoopFlow) {
        let iter_flow = eval_loop_expr(&self.iterable, state);
        let Some(mut pre_loop_state) = iter_flow.normal else {
            let mut fallback_header = self.iterable.type_info(state).state;
            canonicalize_type_state(&mut fallback_header);
            return (
                fallback_header,
                LoopFlow {
                    normal: None,
                    break_state: iter_flow.break_state,
                    continue_state: iter_flow.continue_state,
                    result: TypeDef::never()
                        .maybe_fallible(iter_flow.result.is_fallible())
                        .with_returns(iter_flow.result.returns().clone()),
                },
            );
        };

        canonicalize_type_state(&mut pre_loop_state);
        let mut current_state = pre_loop_state.clone();
        let mut accumulated_break_state: Option<TypeState> = None;

        let iterable_kind = iter_flow.result.kind();
        let mut fallible = iter_flow.result.is_fallible();
        let mut final_returns = iter_flow.result.returns().clone();
        let mut iterations = 0;

        loop {
            let mut iter_state = current_state.clone();
            let pattern_idents = bind_for_pattern(&self.pattern, iterable_kind, &mut iter_state);

            let flow = eval_loop_block(&self.block, &iter_state);
            fallible |= flow.result.is_fallible();
            final_returns = final_returns.union(flow.result.returns().clone());

            let back_edge_state = merge_opt_state(flow.normal, flow.continue_state).map(|mut s| {
                restore_pattern_variables(&mut s, &pre_loop_state, &pattern_idents);
                canonicalize_type_state(&mut s);
                s
            });

            let iter_break_state = flow.break_state.map(|mut s| {
                restore_pattern_variables(&mut s, &pre_loop_state, &pattern_idents);
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

            if next_state == current_state {
                break;
            }

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

        let header_state = current_state.clone();
        let mut post_loop_normal = merge_opt_state(Some(current_state), accumulated_break_state)
            .expect("at least current_state")
            .merge(pre_loop_state);
        canonicalize_type_state(&mut post_loop_normal);

        (
            header_state,
            LoopFlow {
                normal: Some(post_loop_normal),
                break_state: iter_flow.break_state,
                continue_state: iter_flow.continue_state,
                result: TypeDef::null()
                    .maybe_fallible(fallible)
                    .with_returns(final_returns),
            },
        )
    }

    pub(crate) fn resolve_loop_type_info(&self, state: &TypeState) -> (TypeState, TypeInfo) {
        let (header_state, flow) = self.eval_for_in_loop(state);
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

fn merge_opt_state(a: Option<TypeState>, b: Option<TypeState>) -> Option<TypeState> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.merge(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn expr_contains_loop_jump(expr: &Expr) -> bool {
    use crate::compiler::expression::{container::Variant, query::Target, unary};

    match expr {
        Expr::Break(_) | Expr::Continue(_) => true,
        // A nested `for` loop captures jumps from its body, but jumps inside its
        // `iterable` still target the enclosing `for` loop.
        Expr::For(for_expr) => expr_contains_loop_jump(&for_expr.iterable),
        Expr::Literal(_) | Expr::Variable(_) | Expr::Noop(_) => false,
        Expr::Container(container) => match &container.variant {
            Variant::Group(group) => expr_contains_loop_jump(group.inner_expr()),
            Variant::Block(block) => block.exprs().iter().any(expr_contains_loop_jump),
            Variant::Array(array) => array.iter().any(expr_contains_loop_jump),
            Variant::Object(object) => object.values().any(expr_contains_loop_jump),
        },
        Expr::IfStatement(if_stmt) => {
            if_stmt
                .predicate
                .block()
                .exprs()
                .iter()
                .any(expr_contains_loop_jump)
                || if_stmt.if_block.exprs().iter().any(expr_contains_loop_jump)
                || if_stmt
                    .else_block
                    .as_ref()
                    .is_some_and(|b| b.exprs().iter().any(expr_contains_loop_jump))
        }
        Expr::Op(op) => expr_contains_loop_jump(&op.lhs) || expr_contains_loop_jump(&op.rhs),
        Expr::Assignment(assignment) => match assignment.variant() {
            crate::compiler::expression::assignment::Variant::Single { expr, .. }
            | crate::compiler::expression::assignment::Variant::Infallible { expr, .. } => {
                expr_contains_loop_jump(expr)
            }
        },
        Expr::Query(query) => match query.target() {
            Target::Internal(_) | Target::External(_) => false,
            Target::Container(container) => {
                expr_contains_loop_jump(&Expr::Container(container.clone()))
            }
            Target::FunctionCall(call) => call
                .arguments()
                .iter()
                .any(|arg| expr_contains_loop_jump(arg.inner().expr())),
        },
        // Function closures do not jump to an enclosing `for` loop, only arguments can.
        Expr::FunctionCall(call) => call
            .arguments()
            .iter()
            .any(|arg| expr_contains_loop_jump(arg.inner().expr())),
        Expr::Unary(unary) => match unary.variant() {
            unary::Variant::Not(not) => expr_contains_loop_jump(not.inner_expr()),
        },
        Expr::Abort(abort) => abort.message().is_some_and(expr_contains_loop_jump),
        Expr::Return(ret) => expr_contains_loop_jump(ret.expr()),
    }
}

fn eval_loop_block(block: &Block, state: &TypeState) -> LoopFlow {
    if !block.exprs().iter().any(expr_contains_loop_jump) {
        let info = block.type_info(state);
        return LoopFlow {
            normal: Some(info.state),
            break_state: None,
            continue_state: None,
            result: info.result,
        };
    }

    let parent_locals = state.local.clone();
    let mut current_normal = Some(state.clone());
    let mut break_state = None;
    let mut continue_state = None;
    let mut result = TypeDef::null();
    let mut fallible = false;
    let mut returns = Kind::never();
    let mut after_never_expression = false;

    for expr in block.exprs() {
        let Some(ref active_state) = current_normal else {
            break;
        };
        let flow = eval_loop_expr(expr, active_state);
        result = flow.result;

        if !after_never_expression && result.is_fallible() {
            fallible = true;
        }
        if result.is_never() {
            after_never_expression = true;
        }
        returns.merge_keep(result.returns().clone(), false);

        current_normal = flow.normal;
        break_state = merge_opt_state(break_state, flow.break_state);
        continue_state = merge_opt_state(continue_state, flow.continue_state);
    }

    if block.is_scoped() {
        if let Some(ref mut normal) = current_normal {
            normal.local = parent_locals
                .clone()
                .apply_child_scope(normal.local.clone());
        }
        if let Some(ref mut brk) = break_state {
            brk.local = parent_locals.clone().apply_child_scope(brk.local.clone());
        }
        if let Some(ref mut cont) = continue_state {
            cont.local = parent_locals.apply_child_scope(cont.local.clone());
        }
    }

    LoopFlow {
        normal: current_normal,
        break_state,
        continue_state,
        result: result.maybe_fallible(fallible).with_returns(returns),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn eval_loop_expr(expr: &Expr, state: &TypeState) -> LoopFlow {
    use crate::compiler::expression::{assignment, container::Variant, query::Target, unary};
    use crate::parser::ast::Opcode;
    use std::collections::BTreeMap;

    if !expr_contains_loop_jump(expr) {
        let info = expr.type_info(state);
        return LoopFlow {
            normal: Some(info.state),
            break_state: None,
            continue_state: None,
            result: info.result,
        };
    }

    match expr {
        Expr::Break(_) => LoopFlow {
            normal: None,
            break_state: Some(state.clone()),
            continue_state: None,
            result: TypeDef::never(),
        },
        Expr::Continue(_) => LoopFlow {
            normal: None,
            break_state: None,
            continue_state: Some(state.clone()),
            result: TypeDef::never(),
        },
        Expr::For(for_expr) => for_expr.eval_for_in_loop(state).1,
        Expr::Container(container) => match &container.variant {
            Variant::Group(group) => eval_loop_expr(group.inner_expr(), state),
            Variant::Block(block) => eval_loop_block(block, state),
            Variant::Array(array) => {
                let mut current_normal = Some(state.clone());
                let mut break_state = None;
                let mut continue_state = None;
                let mut type_defs = Vec::new();
                let mut fallible = false;

                for item in array.iter() {
                    let Some(ref active_state) = current_normal else {
                        break;
                    };
                    let flow = eval_loop_expr(item, active_state);
                    let type_def = flow.result.upgrade_undefined();
                    fallible |= type_def.is_fallible();
                    current_normal = flow.normal;
                    break_state = merge_opt_state(break_state, flow.break_state);
                    continue_state = merge_opt_state(continue_state, flow.continue_state);

                    if type_def.is_never() || current_normal.is_none() {
                        return LoopFlow {
                            normal: None,
                            break_state,
                            continue_state,
                            result: TypeDef::never().maybe_fallible(fallible),
                        };
                    }
                    type_defs.push(type_def);
                }

                let returns = type_defs.iter().fold(Kind::never(), |returns, type_def| {
                    returns.union(type_def.returns().clone())
                });
                let collection = type_defs
                    .into_iter()
                    .enumerate()
                    .map(|(index, type_def)| (index.into(), type_def.into()))
                    .collect::<BTreeMap<_, _>>();

                LoopFlow {
                    normal: current_normal,
                    break_state,
                    continue_state,
                    result: TypeDef::array(collection)
                        .maybe_fallible(fallible)
                        .with_returns(returns),
                }
            }
            Variant::Object(object) => {
                let mut current_normal = Some(state.clone());
                let mut break_state = None;
                let mut continue_state = None;
                let mut type_defs = BTreeMap::new();
                let mut fallible = false;
                let mut returns = Kind::never();

                for (k, item) in object.iter() {
                    let Some(ref active_state) = current_normal else {
                        break;
                    };
                    let flow = eval_loop_expr(item, active_state);
                    let type_def = flow.result.upgrade_undefined();
                    returns.merge_keep(type_def.returns().clone(), false);
                    fallible |= type_def.is_fallible();
                    current_normal = flow.normal;
                    break_state = merge_opt_state(break_state, flow.break_state);
                    continue_state = merge_opt_state(continue_state, flow.continue_state);

                    if type_def.is_never() || current_normal.is_none() {
                        return LoopFlow {
                            normal: None,
                            break_state,
                            continue_state,
                            result: TypeDef::never()
                                .maybe_fallible(fallible)
                                .with_returns(returns),
                        };
                    }
                    type_defs.insert(k.clone(), type_def);
                }

                let collection = type_defs
                    .into_iter()
                    .map(|(field, type_def)| (field.into(), type_def.into()))
                    .collect::<BTreeMap<_, _>>();

                LoopFlow {
                    normal: current_normal,
                    break_state,
                    continue_state,
                    result: TypeDef::object(collection)
                        .maybe_fallible(fallible)
                        .with_returns(returns),
                }
            }
        },
        Expr::IfStatement(if_stmt) => {
            let pred_flow = eval_loop_block(if_stmt.predicate.block(), state);
            let returns = pred_flow.result.returns().clone();
            let pred_fallible = pred_flow.result.is_fallible();
            let Some(after_pred_state) = pred_flow.normal else {
                return LoopFlow {
                    normal: None,
                    break_state: pred_flow.break_state,
                    continue_state: pred_flow.continue_state,
                    result: pred_flow.result,
                };
            };

            let if_flow = eval_loop_block(&if_stmt.if_block, &after_pred_state);
            if let Some(else_block) = &if_stmt.else_block {
                let else_flow = eval_loop_block(else_block, &after_pred_state);
                let mut result = if_flow.result.union(else_flow.result);
                if pred_fallible {
                    result = result.fallible();
                }
                result.returns_mut().merge_keep(returns, false);
                LoopFlow {
                    normal: merge_opt_state(if_flow.normal, else_flow.normal),
                    break_state: merge_opt_state(
                        pred_flow.break_state,
                        merge_opt_state(if_flow.break_state, else_flow.break_state),
                    ),
                    continue_state: merge_opt_state(
                        pred_flow.continue_state,
                        merge_opt_state(if_flow.continue_state, else_flow.continue_state),
                    ),
                    result,
                }
            } else {
                let mut result = if_flow.result.or_null();
                if pred_fallible {
                    result = result.fallible();
                }
                result.returns_mut().merge_keep(returns, false);
                LoopFlow {
                    normal: merge_opt_state(if_flow.normal, Some(after_pred_state)),
                    break_state: merge_opt_state(pred_flow.break_state, if_flow.break_state),
                    continue_state: merge_opt_state(
                        pred_flow.continue_state,
                        if_flow.continue_state,
                    ),
                    result,
                }
            }
        }
        Expr::Assignment(assignment) => match assignment.variant() {
            assignment::Variant::Single { target, expr } => {
                let rhs_flow = eval_loop_expr(expr, state);
                let expr_result = rhs_flow.result.clone().impure();
                let normal = rhs_flow.normal.map(|mut s| {
                    let const_value = expr.resolve_constant(&s);
                    target.insert_type_def(&mut s, expr_result.clone(), const_value);
                    s
                });
                LoopFlow {
                    normal,
                    break_state: rhs_flow.break_state,
                    continue_state: rhs_flow.continue_state,
                    result: expr_result,
                }
            }
            assignment::Variant::Infallible {
                ok,
                err,
                expr,
                default,
            } => {
                let rhs_flow = eval_loop_expr(expr, state);
                let expr_result = rhs_flow.result.clone();
                if let Some(mut s) = rhs_flow.normal {
                    let ok_type = expr_result
                        .clone()
                        .union(TypeDef::from(default.kind()))
                        .infallible();
                    let err_type = TypeDef::from(Kind::bytes().or_null());
                    let assignment_result = expr_result.infallible().impure().or_bytes();
                    let const_value = expr.resolve_constant(&s);
                    ok.insert_type_def(&mut s, ok_type, const_value);
                    err.insert_type_def(&mut s, err_type, None);
                    LoopFlow {
                        normal: Some(s),
                        break_state: rhs_flow.break_state,
                        continue_state: rhs_flow.continue_state,
                        result: assignment_result,
                    }
                } else {
                    LoopFlow {
                        normal: None,
                        break_state: rhs_flow.break_state,
                        continue_state: rhs_flow.continue_state,
                        result: expr_result,
                    }
                }
            }
        },
        Expr::Op(op) => {
            let lhs_flow = eval_loop_expr(&op.lhs, state);
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
                            result: TypeDef::boolean(),
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
            let rhs_flow = eval_loop_expr(&op.rhs, &rhs_input_state);
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
                let mut flow = eval_loop_expr(&Expr::Container(container.clone()), state);
                if flow.normal.is_some() {
                    flow.result = flow.result.at_path(query.path());
                }
                flow
            }
            Target::FunctionCall(call) => {
                let mut flow = eval_loop_expr(&Expr::FunctionCall(call.clone()), state);
                if flow.normal.is_some() {
                    flow.result = flow.result.at_path(query.path());
                }
                flow
            }
            Target::Internal(_) | Target::External(_) => unreachable!(),
        },
        Expr::FunctionCall(call) => {
            let num_args = call.arguments().len();
            if num_args == 0 {
                let mut normal_state = state.clone();
                let result = call.apply_post_argument_type_info(&mut normal_state, &[]);
                LoopFlow {
                    normal: Some(normal_state),
                    break_state: None,
                    continue_state: None,
                    result,
                }
            } else if num_args <= 6 {
                // Function implementations may resolve arguments in any order at runtime
                // (e.g. `ContainsFn::resolve` evaluates `value` before `substring` regardless
                // of source order). Explore all reachable subset states so each argument is
                // evaluated at most once per permutation and exit states (`break_state` /
                // `continue_state`) accurately reflect every possible evaluation order.
                let num_masks = 1usize << num_args;
                let mut dp_normal: Vec<Option<TypeState>> = vec![None; num_masks];
                dp_normal[0] = Some(state.clone());

                let mut break_state = None;
                let mut continue_state = None;
                let mut fallible = false;
                let mut returns = Kind::never();
                let mut arg_type_defs: Vec<Option<TypeDef>> = vec![None; num_args];

                for mask in 0..num_masks {
                    let Some(current_state) = dp_normal[mask].clone() else {
                        continue;
                    };
                    for (i, arg) in call.arguments().iter().enumerate() {
                        if (mask & (1usize << i)) != 0 {
                            continue;
                        }
                        let flow = eval_loop_expr(arg.inner().expr(), &current_state);
                        fallible |= flow.result.is_fallible();
                        returns.merge_keep(flow.result.returns().clone(), false);
                        arg_type_defs[i] = Some(match arg_type_defs[i].take() {
                            Some(prev) => prev.union(flow.result),
                            None => flow.result,
                        });
                        break_state = merge_opt_state(break_state, flow.break_state);
                        continue_state = merge_opt_state(continue_state, flow.continue_state);
                        let next_mask = mask | (1usize << i);
                        dp_normal[next_mask] =
                            merge_opt_state(dp_normal[next_mask].take(), flow.normal);
                    }
                }

                if let Some(mut normal_state) = dp_normal[num_masks - 1].take() {
                    let resolved_arg_defs: Vec<TypeDef> = arg_type_defs
                        .into_iter()
                        .map(|opt| opt.unwrap_or_else(TypeDef::never))
                        .collect();
                    let result =
                        call.apply_post_argument_type_info(&mut normal_state, &resolved_arg_defs);
                    LoopFlow {
                        normal: Some(normal_state),
                        break_state,
                        continue_state,
                        result,
                    }
                } else {
                    LoopFlow {
                        normal: None,
                        break_state,
                        continue_state,
                        result: TypeDef::never()
                            .maybe_fallible(fallible)
                            .with_returns(returns),
                    }
                }
            } else {
                let mut reachable_state = state.clone();
                canonicalize_type_state(&mut reachable_state);
                let mut iterations = 0;

                loop {
                    let mut next_reachable = reachable_state.clone();
                    for arg in call.arguments() {
                        let flow = eval_loop_expr(arg.inner().expr(), &reachable_state);
                        if let Some(norm) = flow.normal {
                            next_reachable = next_reachable.merge(norm);
                        }
                    }
                    canonicalize_type_state(&mut next_reachable);

                    if next_reachable == reachable_state {
                        break;
                    }

                    iterations += 1;
                    if iterations >= WIDEN_AFTER_ITERATIONS {
                        widen_differing_states(
                            &reachable_state,
                            &mut next_reachable,
                            iterations >= FALLBACK_WIDEN_ITERATIONS,
                        );
                    }
                    reachable_state = next_reachable;
                }

                let mut break_state = None;
                let mut continue_state = None;
                let mut fallible = false;
                let mut returns = Kind::never();
                let mut arg_type_defs = Vec::with_capacity(num_args);
                let mut any_diverged = false;

                for arg in call.arguments() {
                    let flow = eval_loop_expr(arg.inner().expr(), &reachable_state);
                    fallible |= flow.result.is_fallible();
                    returns.merge_keep(flow.result.returns().clone(), false);
                    any_diverged |= flow.normal.is_none();
                    arg_type_defs.push(flow.result);
                    break_state = merge_opt_state(break_state, flow.break_state);
                    continue_state = merge_opt_state(continue_state, flow.continue_state);
                }

                if any_diverged {
                    LoopFlow {
                        normal: None,
                        break_state,
                        continue_state,
                        result: TypeDef::never()
                            .maybe_fallible(fallible)
                            .with_returns(returns),
                    }
                } else {
                    let mut normal_state = reachable_state;
                    let result =
                        call.apply_post_argument_type_info(&mut normal_state, &arg_type_defs);
                    LoopFlow {
                        normal: Some(normal_state),
                        break_state,
                        continue_state,
                        result,
                    }
                }
            }
        }
        Expr::Unary(unary) => match unary.variant() {
            unary::Variant::Not(not) => {
                let inner_flow = eval_loop_expr(not.inner_expr(), state);
                let base_result = if inner_flow.normal.is_some() {
                    TypeDef::boolean()
                } else {
                    TypeDef::never()
                };
                let result = base_result
                    .maybe_fallible(inner_flow.result.is_fallible())
                    .with_returns(inner_flow.result.returns().clone());
                LoopFlow {
                    normal: inner_flow.normal,
                    break_state: inner_flow.break_state,
                    continue_state: inner_flow.continue_state,
                    result,
                }
            }
        },
        Expr::Abort(abort) => {
            if let Some(msg) = abort.message() {
                let msg_flow = eval_loop_expr(msg, state);
                let returns = msg_flow.result.returns().clone();
                LoopFlow {
                    normal: None,
                    break_state: msg_flow.break_state,
                    continue_state: msg_flow.continue_state,
                    result: TypeDef::never()
                        .maybe_fallible(msg_flow.result.is_fallible())
                        .with_returns(returns),
                }
            } else {
                LoopFlow {
                    normal: None,
                    break_state: None,
                    continue_state: None,
                    result: TypeDef::never(),
                }
            }
        }
        Expr::Return(ret) => {
            let inner_flow = eval_loop_expr(ret.expr(), state);
            let mut returns = inner_flow.result.returns().clone();
            if inner_flow.normal.is_some() {
                returns.merge_keep(inner_flow.result.kind().clone(), false);
            }
            LoopFlow {
                normal: None,
                break_state: inner_flow.break_state,
                continue_state: inner_flow.continue_state,
                result: TypeDef::never()
                    .maybe_fallible(inner_flow.result.is_fallible())
                    .with_returns(returns),
            }
        }
        Expr::Literal(_) | Expr::Variable(_) | Expr::Noop(_) => unreachable!(),
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
            for _ in { return "foo"; [1, 2] } {
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

        // 5. Conditional break inside a function argument or binary operator does not leak onto the back edge
        let fns = crate::stdlib::all();
        let nested_expr_break = r"
            x = [0, 1]
            for i, _ in [1, 2] {
                .tmp1 = to_string({
                    if i == 1 {
                        x = [x, 1]
                        break
                    }
                    i
                })
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
        let res_nested = crate::compiler::compile(nested_expr_break, &fns);
        assert!(
            res_nested.is_ok(),
            "expected conditional break inside function arg and op not to widen x across back edge, got: {:?}",
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

            fn usage(&self) -> &'static str {
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
    fn test_for_function_call_exit_tracks_any_argument_order() {
        let fns = crate::stdlib::all();

        // 1. Named arguments where runtime evaluates `value` before `substring`
        let named_contains_break = r#"
            x = 1
            for _ in [1] {
                .tmp = contains(substring: { if true { break }; "a" }, value: { x = "s"; "a" })
                x = 1
            }
            .y = x + 1
        "#;
        let Err(err1) = crate::compiler::compile(named_contains_break, &fns) else {
            panic!("expected break in `substring` to observe mutation from `value`");
        };
        assert_eq!(err1[0].code, codes::ExprCode::FallibleAssignment as usize);

        // 2. Reverse order: `value` breaks, `substring` mutates `x`
        let reverse_contains_break = r#"
            x = 1
            for _ in [1] {
                .tmp = contains(substring: { x = "s"; "a" }, value: { if true { break }; "a" })
                x = 1
            }
            .y = x + 1
        "#;
        let Err(err2) = crate::compiler::compile(reverse_contains_break, &fns) else {
            panic!("expected break in `value` to conservatively observe mutation from `substring`");
        };
        assert_eq!(err2[0].code, codes::ExprCode::FallibleAssignment as usize);

        // 3. Unconditional break in one argument while another argument mutates `x`
        let uncond_contains_break = r#"
            x = 1
            for _ in [1] {
                .tmp = contains(substring: { break; "a" }, value: { x = "s"; "a" })
                x = 1
            }
            .y = x + 1
        "#;
        let Err(err3) = crate::compiler::compile(uncond_contains_break, &fns) else {
            panic!("expected unconditional break in `substring` to observe `value` and skip x = 1");
        };
        assert_eq!(err3[0].code, codes::ExprCode::FallibleAssignment as usize);

        // 4. Conditional continue in one argument while another argument mutates `x`
        let named_contains_continue = r#"
            x = 1
            for _ in [1, 2] {
                .y = x + 1
                .tmp = contains(substring: { if true { continue }; "a" }, value: { x = "s"; "a" })
                x = 1
            }
        "#;
        let Err(err4) = crate::compiler::compile(named_contains_continue, &fns) else {
            panic!("expected continue in `substring` to propagate `x = \"s\"` along back-edge");
        };
        assert_eq!(err4[0].code, codes::ExprCode::FallibleAssignment as usize);
    }
}
