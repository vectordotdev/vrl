use std::fmt;

use crate::value::Value;

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    expression::{Block, Predicate, Resolved},
    value::VrlValueConvert,
    Context, Expression,
};

#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub predicate: Predicate,
    pub if_block: Block,
    pub else_block: Option<Block>,
}

impl Expression for IfStatement {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let predicate = self.predicate.resolve(ctx)?.try_boolean()?;

        match predicate {
            true => self.if_block.resolve(ctx),
            false => self
                .else_block
                .as_ref()
                .map_or(Ok(Value::Null), |block| block.resolve(ctx)),
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let mut state = state.clone();
        let predicate_info = self.predicate.apply_type_info(&mut state);

        let if_info = self.if_block.type_info(&state);

        if let Some(else_block) = &self.else_block {
            let else_info = else_block.type_info(&state);

            // final state will be from either the "if" or "else" block, but not the original
            let final_state = if_info.state.merge(else_info.state);

            // result is from either "if" or the "else" block
            let mut result = if_info.result.union(else_info.result);

            // predicate can also return
            result
                .returns_mut()
                .merge_keep(predicate_info.returns().clone(), false);

            TypeInfo::new(final_state, result)
        } else {
            // state changes from the "if block" are optional, so merge it with the original
            let final_state = if_info.state.merge(state);

            // if the predicate is false, "null" is returned.
            let mut result = if_info.result.or_null();

            // predicate can also return
            result
                .returns_mut()
                .merge_keep(predicate_info.returns().clone(), false);

            TypeInfo::new(final_state, result)
        }
    }
}

impl fmt::Display for IfStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("if ")?;
        self.predicate.fmt(f)?;
        f.write_str(" ")?;
        self.if_block.fmt(f)?;

        if let Some(alt) = &self.else_block {
            f.write_str(" else")?;
            alt.fmt(f)?;
        }

        Ok(())
    }
}
