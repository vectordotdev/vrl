use std::fmt;

use crate::value::{Kind, Value};

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    Context, Expression,
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

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        // Match nested else-if semantics: arm i is typed after predicates 0..=i
        // have been applied (previous predicates ran and were false at runtime,
        // but their type-level side effects still apply).
        let mut running = state.clone();
        let mut returns = Kind::never();
        let mut arm_states: Vec<TypeState> = Vec::with_capacity(self.arms.len());
        let mut result_def: Option<crate::compiler::TypeDef> = None;
        // Locals present after arm 0's predicate (inbound + first-pred assigns).
        // Used below to drop bindings first introduced by later predicates.
        let mut locals_after_first_predicate = None;

        for arm in &self.arms {
            let predicate_info = arm.predicate.apply_type_info(&mut running);
            returns.merge_keep(predicate_info.returns().clone(), false);

            if locals_after_first_predicate.is_none() {
                locals_after_first_predicate = Some(running.local.clone());
            }

            let arm_info = arm.block.type_info(&running);
            result_def = Some(match result_def {
                None => arm_info.result,
                Some(prev) => prev.union(arm_info.result),
            });
            arm_states.push(arm_info.state);
        }

        let mut result = result_def.expect("at least one arm");

        let mut final_state = if let Some(else_block) = &self.else_block {
            let else_info = else_block.type_info(&running);
            result = result.union(else_info.result);

            arm_states
                .into_iter()
                .fold(else_info.state, |acc, arm_state| acc.merge(arm_state))
        } else {
            // All predicates false → null, and state is the post-predicate state
            // merged with every arm (same as nested `if` without `else`).
            result = result.or_null();
            arm_states
                .into_iter()
                .fold(running, |acc, arm_state| acc.merge(arm_state))
        };

        // Flattened else-if peels parser `else { if }` wrappers. Those Blocks
        // used `apply_child_scope`, so locals first assigned in a later
        // predicate did not escape the chain. Re-apply that rule: keep/merge
        // updates to locals that already existed after arm 0's predicate;
        // drop bindings introduced only by later predicates (they are not
        // definite when an earlier arm matched).
        if let Some(baseline) = locals_after_first_predicate {
            final_state.local = baseline.apply_child_scope(final_state.local);
        }

        result.returns_mut().merge_keep(returns, false);
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
