use std::fmt;

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    Context, Expression, ExpressionError, TypeDef,
    expression::{Expr, Resolved},
};
use crate::value::Kind;

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    inner: Vec<Expr>,

    // false - This is just an inline block of code
    // true - This is a block of code nested in a child scope
    new_scope: bool,
}

impl Block {
    #[must_use]
    fn new(inner: Vec<Expr>, new_scope: bool) -> Self {
        Self { inner, new_scope }
    }

    #[must_use]
    pub fn new_scoped(inner: Vec<Expr>) -> Self {
        Self::new(inner, true)
    }

    #[must_use]
    pub fn new_inline(inner: Vec<Expr>) -> Self {
        Self::new(inner, false)
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<Expr> {
        self.inner
    }

    #[must_use]
    pub fn exprs(&self) -> &Vec<Expr> {
        &self.inner
    }
}

impl Expression for Block {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        // Variables are checked at compile-time to ensure only variables
        // in scope can be accessed here, so it doesn't need to be checked at runtime.
        let (last, other) = self.inner.split_last().expect("at least one expression");

        other
            .iter()
            .try_for_each(|expr| expr.resolve_discard(ctx))?;

        last.resolve(ctx)
    }

    fn resolve_discard(&self, ctx: &mut Context) -> Result<(), ExpressionError> {
        self.inner
            .iter()
            .try_for_each(|expr| expr.resolve_discard(ctx))
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let parent_locals = state.local.clone();

        let mut state = state.clone();
        let mut result = TypeDef::null();
        let mut fallible = false;
        let mut returns = Kind::never();
        let mut after_never_expression = false;

        for expr in &self.inner {
            result = expr.apply_type_info(&mut state);

            if !after_never_expression && result.is_fallible() {
                fallible = true;
            }
            if result.is_never() {
                after_never_expression = true;
            }
            returns.merge_keep(result.returns().clone(), false);
        }

        if self.new_scope {
            state.local = parent_locals.apply_child_scope(state.local);
        }

        TypeInfo::new(state, result.maybe_fallible(fallible).with_returns(returns))
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{\n")?;

        let mut iter = self.inner.iter().peekable();
        while let Some(expr) = iter.next() {
            f.write_str("\t")?;
            expr.fmt(f)?;
            if iter.peek().is_some() {
                f.write_str("\n")?;
            }
        }

        f.write_str("\n}")
    }
}

#[cfg(all(test, feature = "stdlib"))]
mod tests {
    use std::ops::ControlFlow;

    use crate::compiler::expression::assignment::tests::SpyTarget;
    use crate::compiler::runtime::{Runtime, Terminate};
    use crate::compiler::state::RuntimeState;
    use crate::compiler::{Context, ExpressionError, Target, TimeZone, compile};
    use crate::owned_value_path;
    use crate::path::OwnedTargetPath;
    use crate::value::Value;

    #[test]
    fn non_final_assignment_in_block_moves_without_cloning() {
        let compiled = compile(r#".a = upcase("hello"); "done""#, &crate::stdlib::all()).unwrap();

        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);

        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from("done")));
        assert_eq!(spy.inserted_unique, vec![true]);
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from("HELLO")))
        );
    }

    #[test]
    fn nested_block_group_and_if_propagate_resolve_discard() {
        let src = indoc::indoc! {r#"
            { .a = upcase("one") }
            (.b = upcase("two"))
            if true { .c = upcase("three") }
            if false { .d = upcase("unused") } else { .e = upcase("four") }
            null
        "#};
        let compiled = compile(src, &crate::stdlib::all()).unwrap();

        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);

        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::Null));
        assert_eq!(spy.inserted_unique, vec![true, true, true, true]);
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from("ONE")))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("b"))),
            Ok(Some(&Value::from("TWO")))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("c"))),
            Ok(Some(&Value::from("THREE")))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("d"))),
            Ok(None)
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("e"))),
            Ok(Some(&Value::from("FOUR")))
        );
    }

    #[test]
    fn if_without_else_false_in_discard_position() {
        let compiled = compile(
            r#"if false { .a = upcase("nope") }; 123"#,
            &crate::stdlib::all(),
        )
        .unwrap();

        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);

        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from(123)));
        assert!(spy.inserted_unique.is_empty());
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(None)
        );
    }

    #[test]
    fn final_and_value_position_assignments_still_return_value() {
        let tz = TimeZone::default();

        // Single-statement assignment.
        let compiled = compile(r#".a = upcase("hello")"#, &crate::stdlib::all()).unwrap();
        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);
        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from("HELLO")));
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from("HELLO")))
        );

        // Multi-statement with assignment in final position.
        let compiled = compile(r#".a = 1; .b = upcase("world")"#, &crate::stdlib::all()).unwrap();
        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);
        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from("WORLD")));
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from(1)))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("b"))),
            Ok(Some(&Value::from("WORLD")))
        );

        // Chained / parenthesized assignment in value position.
        let compiled = compile(r#".a = (.b = upcase("chain"))"#, &crate::stdlib::all()).unwrap();
        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);
        assert_eq!(compiled.program.resolve(&mut ctx), Ok(Value::from("CHAIN")));
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from("CHAIN")))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("b"))),
            Ok(Some(&Value::from("CHAIN")))
        );

        // Final `if` branch assignment.
        let compiled = compile(
            r#"if true { .a = upcase("branch") }"#,
            &crate::stdlib::all(),
        )
        .unwrap();
        let mut spy = SpyTarget::new();
        let mut state = RuntimeState::default();
        let mut ctx = Context::new(&mut spy, &mut state, &tz);
        assert_eq!(
            compiled.program.resolve(&mut ctx),
            Ok(Value::from("BRANCH"))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("a"))),
            Ok(Some(&Value::from("BRANCH")))
        );
    }

    #[test]
    fn return_and_abort_inside_discarded_block_and_if_short_circuit() {
        let tz = TimeZone::default();

        let compiled_return = compile(
            "if true { return 42 }; .after = true",
            &crate::stdlib::all(),
        )
        .unwrap();
        let mut spy = SpyTarget::new();
        let mut runtime = Runtime::default();
        assert_eq!(
            runtime.resolve(&mut spy, &compiled_return.program, &tz),
            Ok(Value::from(42))
        );
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("after"))),
            Ok(None)
        );

        let compiled_abort = compile(
            r#"if true { abort "halt" }; .after = true"#,
            &crate::stdlib::all(),
        )
        .unwrap();
        let mut spy = SpyTarget::new();
        let mut runtime = Runtime::default();
        assert!(matches!(
            runtime.resolve(&mut spy, &compiled_abort.program, &tz),
            Err(Terminate::Abort(ExpressionError::Abort { message: Some(ref msg), .. })) if msg == "halt"
        ));
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("after"))),
            Ok(None)
        );
    }

    #[test]
    fn execution_control_interrupts_non_final_assignment_in_block() {
        let compiled = compile(".a = 1; .b = 2; .c = 3; null", &crate::stdlib::all()).unwrap();

        let mut spy = SpyTarget::new();
        let mut runtime = Runtime::default();
        let tz = TimeZone::default();
        let mut checkpoints = 0usize;
        let mut control = || {
            checkpoints += 1;
            if checkpoints >= 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };

        assert_eq!(
            runtime.resolve_with_control(&mut spy, &compiled.program, &tz, &mut control),
            Err(Terminate::Interrupted)
        );
        assert_eq!(checkpoints, 2);
        assert_eq!(
            spy.target_get(&OwnedTargetPath::event(owned_value_path!("c"))),
            Ok(None)
        );
    }
}
