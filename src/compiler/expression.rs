use std::fmt;

use dyn_clone::{clone_trait_object, DynClone};

pub use abort::Abort;
pub use array::Array;
pub use assignment::Assignment;
pub use block::Block;
pub use container::{Container, Variant};
pub use function::FunctionExpression;
pub use function_argument::FunctionArgument;
pub use function_call::FunctionCall;
pub use group::Group;
pub use if_statement::IfStatement;
pub use literal::Literal;
pub use noop::Noop;
pub use not::Not;
pub use object::Object;
pub use op::Op;
pub use predicate::Predicate;
pub use query::{Query, Target};
pub use r#return::Return;
pub use unary::Unary;
pub use variable::Variable;

use crate::value::Value;

use super::state::{TypeInfo, TypeState};
use super::{Context, TypeDef};
pub use super::{ExpressionError, Resolved};

mod abort;
mod array;
mod block;
mod function_argument;
mod group;
mod if_statement;
mod levenstein;
mod noop;
mod not;
mod object;
mod op;
mod r#return;
pub(crate) mod unary;
mod variable;

pub(crate) mod assignment;
pub(crate) mod container;
pub(crate) mod function;
pub(crate) mod function_call;
pub(crate) mod literal;
pub(crate) mod predicate;
pub mod query;

pub trait Expression: Send + Sync + fmt::Debug + DynClone {
    /// Resolve an expression to a concrete [`Value`].
    ///
    /// This method is executed at runtime.
    ///
    /// An expression is allowed to fail, which aborts the running program.
    fn resolve(&self, ctx: &mut Context) -> Resolved;

    /// Resolve an expression to a value without any context, if possible.
    /// This attempts to resolve expressions using only compile-time information.
    ///
    /// This returns `Some` for static expressions, or `None` for dynamic expressions.
    fn resolve_constant(&self, _state: &TypeState) -> Option<Value> {
        None
    }

    /// Resolve an expression to its [`TypeDef`] type definition.
    /// This must be called with the _initial_ `TypeState`.
    ///
    /// Consider calling `type_info` instead if you want to capture changes in the type
    /// state from side-effects.
    fn type_def(&self, state: &TypeState) -> TypeDef {
        self.type_info(state).result
    }

    /// Calculates the type state after an expression resolves, including the expression result itself.
    /// This must be called with the _initial_ `TypeState`.
    ///
    /// Consider using `apply_type_info` instead if you want to just access
    /// the expr result type, while updating an existing state.
    fn type_info(&self, state: &TypeState) -> TypeInfo;

    /// Applies state changes from the expression to the given state, and
    /// returns the result type.
    fn apply_type_info(&self, state: &mut TypeState) -> TypeDef {
        let new_info = self.type_info(state);
        *state = new_info.state;
        new_info.result
    }

    /// Format the expression into a consistent style.
    ///
    /// This defaults to not formatting, so that function implementations don't
    /// need to care about formatting (this is handled by the internal function
    /// call expression).
    fn format(&self) -> Option<String> {
        None
    }
}

clone_trait_object!(Expression);

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Container(Container),
    IfStatement(IfStatement),
    Op(Op),
    Assignment(Assignment),
    Query(Query),
    FunctionCall(FunctionCall),
    Variable(Variable),
    Noop(Noop),
    Unary(Unary),
    Abort(Abort),
    Return(Return),
}

impl Expr {
    pub fn as_str(&self) -> &str {
        use container::Variant::{Array, Block, Group, Object};
        use Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Noop, Op, Query,
            Return, Unary, Variable,
        };

        match self {
            Literal(..) => "literal",
            Container(v) => match &v.variant {
                Group(..) => "group",
                Block(..) => "block",
                Array(..) => "array",
                Object(..) => "object",
            },
            IfStatement(..) => "if-statement",
            Op(..) => "operation",
            Assignment(..) => "assignment",
            Query(..) => "query",
            FunctionCall(..) => "function call",
            Variable(..) => "variable call",
            Noop(..) => "noop",
            Unary(..) => "unary operation",
            Abort(..) => "abort operation",
            Return(..) => "return",
        }
    }

    pub fn as_literal(
        &self,
        keyword: &'static str,
        state: &TypeState,
    ) -> Result<Value, super::function::Error> {
        match self.resolve_constant(state) {
            Some(value) => Ok(value),
            None => Err(super::function::Error::UnexpectedExpression {
                keyword,
                expected: "literal",
                expr: self.clone(),
            }),
        }
    }

    pub fn as_enum(
        &self,
        keyword: &'static str,
        variants: Vec<Value>,
        state: &TypeState,
    ) -> Result<Value, super::function::Error> {
        let value = self.as_literal(keyword, state)?;
        variants.iter().find(|v| **v == value).cloned().ok_or(
            super::function::Error::InvalidEnumVariant {
                keyword,
                value,
                variants,
            },
        )
    }
}

impl Expression for Expr {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        use Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Noop, Op, Query,
            Return, Unary, Variable,
        };

        match self {
            Literal(v) => v.resolve(ctx),
            Container(v) => v.resolve(ctx),
            IfStatement(v) => v.resolve(ctx),
            Op(v) => v.resolve(ctx),
            Assignment(v) => v.resolve(ctx),
            Query(v) => v.resolve(ctx),
            FunctionCall(v) => v.resolve(ctx),
            Variable(v) => v.resolve(ctx),
            Noop(v) => v.resolve(ctx),
            Unary(v) => v.resolve(ctx),
            Abort(v) => v.resolve(ctx),
            Return(v) => v.resolve(ctx),
        }
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        use Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Noop, Op, Query,
            Return, Unary, Variable,
        };

        match self {
            Literal(v) => Expression::resolve_constant(v, state),
            Container(v) => Expression::resolve_constant(v, state),
            IfStatement(v) => Expression::resolve_constant(v, state),
            Op(v) => Expression::resolve_constant(v, state),
            Assignment(v) => Expression::resolve_constant(v, state),
            Query(v) => Expression::resolve_constant(v, state),
            FunctionCall(v) => Expression::resolve_constant(v, state),
            Variable(v) => Expression::resolve_constant(v, state),
            Noop(v) => Expression::resolve_constant(v, state),
            Unary(v) => Expression::resolve_constant(v, state),
            Abort(v) => Expression::resolve_constant(v, state),
            Return(v) => Expression::resolve_constant(v, state),
        }
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        use Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Noop, Op, Query,
            Return, Unary, Variable,
        };

        match self {
            Literal(v) => v.type_info(state),
            Container(v) => v.type_info(state),
            IfStatement(v) => v.type_info(state),
            Op(v) => v.type_info(state),
            Assignment(v) => v.type_info(state),
            Query(v) => v.type_info(state),
            FunctionCall(v) => v.type_info(state),
            Variable(v) => v.type_info(state),
            Noop(v) => v.type_info(state),
            Unary(v) => v.type_info(state),
            Abort(v) => v.type_info(state),
            Return(v) => v.type_info(state),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Noop, Op, Query,
            Return, Unary, Variable,
        };

        match self {
            Literal(v) => v.fmt(f),
            Container(v) => v.fmt(f),
            IfStatement(v) => v.fmt(f),
            Op(v) => v.fmt(f),
            Assignment(v) => v.fmt(f),
            Query(v) => v.fmt(f),
            FunctionCall(v) => v.fmt(f),
            Variable(v) => v.fmt(f),
            Noop(v) => v.fmt(f),
            Unary(v) => v.fmt(f),
            Abort(v) => v.fmt(f),
            Return(v) => v.fmt(f),
        }
    }
}

// -----------------------------------------------------------------------------

impl From<Literal> for Expr {
    fn from(literal: Literal) -> Self {
        Expr::Literal(literal)
    }
}

impl From<Container> for Expr {
    fn from(container: Container) -> Self {
        Expr::Container(container)
    }
}

impl From<IfStatement> for Expr {
    fn from(if_statement: IfStatement) -> Self {
        Expr::IfStatement(if_statement)
    }
}

impl From<Op> for Expr {
    fn from(op: Op) -> Self {
        Expr::Op(op)
    }
}

impl From<Assignment> for Expr {
    fn from(assignment: Assignment) -> Self {
        Expr::Assignment(assignment)
    }
}

impl From<Query> for Expr {
    fn from(query: Query) -> Self {
        Expr::Query(query)
    }
}

impl From<FunctionCall> for Expr {
    fn from(function_call: FunctionCall) -> Self {
        Expr::FunctionCall(function_call)
    }
}

impl From<Variable> for Expr {
    fn from(variable: Variable) -> Self {
        Expr::Variable(variable)
    }
}

impl From<Noop> for Expr {
    fn from(noop: Noop) -> Self {
        Expr::Noop(noop)
    }
}

impl From<Unary> for Expr {
    fn from(unary: Unary) -> Self {
        Expr::Unary(unary)
    }
}

impl From<Abort> for Expr {
    fn from(abort: Abort) -> Self {
        Expr::Abort(abort)
    }
}

impl From<Return> for Expr {
    fn from(r#return: Return) -> Self {
        Expr::Return(r#return)
    }
}

impl From<Value> for Expr {
    fn from(value: Value) -> Self {
        use std::collections::BTreeMap;

        use crate::value::Value::{
            Array, Boolean, Bytes, Float, Integer, Null, Object, Regex, Timestamp,
        };

        match value {
            Bytes(v) => Literal::from(v).into(),
            Integer(v) => Literal::from(v).into(),
            Float(v) => Literal::from(v).into(),
            Boolean(v) => Literal::from(v).into(),
            Object(v) => {
                let object = super::expression::Object::from(
                    v.into_iter()
                        .map(|(k, v)| (k, v.into()))
                        .collect::<BTreeMap<_, _>>(),
                );

                Container::new(container::Variant::from(object)).into()
            }
            Array(v) => {
                let array = super::expression::Array::from(
                    v.into_iter().map(Expr::from).collect::<Vec<_>>(),
                );

                Container::new(container::Variant::from(array)).into()
            }
            Timestamp(v) => Literal::from(v).into(),
            Regex(v) => Literal::from(v).into(),
            Null => Literal::from(()).into(),
        }
    }
}
