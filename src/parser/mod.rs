#![deny(
    clippy::all,
    unreachable_pub,
    unused_allocation,
    unused_extern_crates,
    unused_assignments,
    unused_comparisons
)]
use std::borrow::ToOwned;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(
    #[allow(
        warnings,
        clippy::all,
        clippy::pedantic,
        unreachable_pub,
        unused_allocation,
        unused_extern_crates,
        unused_assignments,
        unused_comparisons
    )]
    parser,
    "/parser/parser.rs"
);

pub mod ast;
mod lex;
pub mod template_string;

pub use crate::diagnostic::Span;
pub use ast::{Literal, Program};
pub use lex::{Error, Token};

/// Parses a VRL program.
///
/// # Errors
///
/// Returns a parser or lexer error when the input is not a valid VRL program.
pub fn parse(input: impl AsRef<str>) -> Result<Program, Error> {
    let lexer = lex::Lexer::new(input.as_ref());

    parser::ProgramParser::new()
        .parse(input.as_ref(), lexer)
        .map_err(|source| match source {
            lalrpop_util::ParseError::User { error } => error,
            source => Error::ParseError {
                span: Span::new(0, input.as_ref().len()),
                source: source
                    .map_token(|t| t.map(ToOwned::to_owned))
                    .map_error(|err| err.to_string()),
                dropped_tokens: vec![],
            },
        })
}

/// Parses a single VRL literal.
///
/// # Errors
///
/// Returns a parser or lexer error when the input is not a valid VRL literal.
pub fn parse_literal(input: impl AsRef<str>) -> Result<Literal, Error> {
    let lexer = lex::Lexer::new(input.as_ref());

    parser::LiteralParser::new()
        .parse(input.as_ref(), lexer)
        .map_err(|source| Error::ParseError {
            span: Span::new(0, input.as_ref().len()),
            source: source
                .map_token(|t| t.map(ToOwned::to_owned))
                .map_error(|err| err.to_string()),
            dropped_tokens: vec![],
        })
}
