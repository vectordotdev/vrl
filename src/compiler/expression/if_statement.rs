use std::fmt;

use crate::value::{Kind, Value};

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    Context, Expression, ExpressionError,
    expression::{Block, Predicate, Resolved},
    value::VrlValueConvert,
};

#[derive(Debug, Clone, PartialEq)]
pub struct IfArm {
    pub predicate: Predicate,
    pub block: Block,
}

/// `if` / `else if` / `else` as a flat multi-arm node.
///
/// The parser still emits nested `else { if ... }` for `else if`, but the compiler
/// peels that chain into [`IfStatement::arms`] so typecheck is O(arms) instead of
/// re-walking nested suffixes (quadratic in arm count).
#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub arms: Vec<IfArm>,
    pub else_block: Option<Block>,
}

impl IfStatement {
    /// # Panics
    ///
    /// Panics if `arms` is empty.
    #[must_use]
    pub fn new(arms: Vec<IfArm>, else_block: Option<Block>) -> Self {
        assert!(!arms.is_empty(), "if statement requires at least one arm");
        Self { arms, else_block }
    }
}

impl Expression for IfStatement {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        for arm in &self.arms {
            let predicate = arm.predicate.resolve(ctx)?.try_boolean()?;
            if predicate {
                return arm.block.resolve(ctx);
            }
        }

        self.else_block
            .as_ref()
            .map_or(Ok(Value::Null), |block| block.resolve(ctx))
    }

    fn resolve_unused(&self, ctx: &mut Context) -> Result<(), ExpressionError> {
        for arm in &self.arms {
            let predicate = arm.predicate.resolve(ctx)?.try_boolean()?;
            if predicate {
                return arm.block.resolve_unused(ctx);
            }
        }

        self.else_block
            .as_ref()
            .map_or(Ok(()), |block| block.resolve_unused(ctx))
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        // Match nested else-if semantics: arm i is typed after predicates 0..=i
        // have been applied (previous predicates ran and were false at runtime,
        // but their type-level side effects still apply).
        let mut running = state.clone();
        let mut arm_infos: Vec<(TypeInfo, Kind)> = Vec::with_capacity(self.arms.len());
        // Locals present after arm 0's predicate (inbound + first-pred assigns).
        // Used below to drop bindings first introduced by later predicates.
        let mut locals_after_first_predicate = None;

        for arm in &self.arms {
            let predicate_info = arm.predicate.apply_type_info(&mut running);

            if locals_after_first_predicate.is_none() {
                locals_after_first_predicate = Some(running.local.clone());
            }

            let arm_info = arm.block.type_info(&running);
            arm_infos.push((arm_info, predicate_info.returns().clone()));
        }

        // Combine right to left, exactly as nested `if a {} else { if b {} else {} }`
        // did: arm i joins the already combined arms i+1.. and else. Type unions
        // and state merges are not associative (an infinite unknown overwrites an
        // exact one), so a left-to-right fold would infer different types.
        let mut arm_infos = arm_infos.into_iter().rev();
        let (last, last_predicate_returns) = arm_infos.next().expect("at least one arm");

        let (mut final_state, mut result) = if let Some(else_block) = &self.else_block {
            let else_info = else_block.type_info(&running);
            (
                last.state.merge(else_info.state),
                last.result.union(else_info.result),
            )
        } else {
            // All predicates false: the result is null and the state is the one
            // after the last predicate (same as nested `if` without `else`).
            (last.state.merge(running), last.result.or_null())
        };
        result
            .returns_mut()
            .merge_keep(last_predicate_returns, false);

        for (arm_info, predicate_returns) in arm_infos {
            final_state = arm_info.state.merge(final_state);
            result = arm_info.result.union(result);
            result.returns_mut().merge_keep(predicate_returns, false);
        }

        // Flattened else-if peels parser `else { if }` wrappers. Those Blocks
        // used `apply_child_scope`, so locals first assigned in a later
        // predicate did not escape the chain. Re-apply that rule: keep/merge
        // updates to locals that already existed after arm 0's predicate;
        // drop bindings introduced only by later predicates (they are not
        // definite when an earlier arm matched). Merges are per binding, so
        // scoping once here equals scoping at each nested level.
        if let Some(baseline) = locals_after_first_predicate {
            final_state.local = baseline.apply_child_scope(final_state.local);
        }

        TypeInfo::new(final_state, result)
    }
}

impl fmt::Display for IfStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut arms = self.arms.iter();
        let first = arms.next().expect("at least one arm");
        f.write_str("if ")?;
        first.predicate.fmt(f)?;
        f.write_str(" ")?;
        first.block.fmt(f)?;

        for arm in arms {
            f.write_str(" else if ")?;
            arm.predicate.fmt(f)?;
            f.write_str(" ")?;
            arm.block.fmt(f)?;
        }

        if let Some(alt) = &self.else_block {
            f.write_str(" else")?;
            alt.fmt(f)?;
        }

        Ok(())
    }
}
