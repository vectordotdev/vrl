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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_for_single_variable() {
        let source = "for x in [1, 2] { .foo = x }";
        let parsed = parse(source).expect("should parse single variable for loop");
        assert_eq!(parsed.len(), 1);
        let ast::RootExpr::Expr(ref expr) = parsed[0].node else {
            panic!("expected RootExpr::Expr, got {:?}", parsed[0].node);
        };
        let ast::Expr::For(ref for_stmt) = expr.node else {
            panic!("expected Expr::For, got {:?}", expr.node);
        };
        let ast::ForPattern::Single(ref ident) = for_stmt.pattern else {
            panic!("expected ForPattern::Single, got {:?}", for_stmt.pattern);
        };
        assert_eq!(&ident.0, "x");
        assert_eq!(for_stmt.block.0.len(), 1);
    }

    #[test]
    fn test_parse_for_key_value() {
        let source = "for k, v in { \"a\": 1 } { .foo = v }";
        let parsed = parse(source).expect("should parse key-value for loop");
        assert_eq!(parsed.len(), 1);
        let ast::RootExpr::Expr(ref expr) = parsed[0].node else {
            panic!("expected RootExpr::Expr, got {:?}", parsed[0].node);
        };
        let ast::Expr::For(ref for_stmt) = expr.node else {
            panic!("expected Expr::For, got {:?}", expr.node);
        };
        let ast::ForPattern::KeyValue(ref key, ref value) = for_stmt.pattern else {
            panic!("expected ForPattern::KeyValue, got {:?}", for_stmt.pattern);
        };
        assert_eq!(&key.0, "k");
        assert_eq!(&value.0, "v");
        assert_eq!(for_stmt.block.0.len(), 1);
    }

    #[test]
    fn test_parse_for_with_break_and_continue() {
        let source = "for x in [1, 2] { if x == 1 { continue } else { break } }";
        let parsed = parse(source).expect("should parse for loop with break and continue");
        assert_eq!(parsed.len(), 1);
        let ast::RootExpr::Expr(ref expr) = parsed[0].node else {
            panic!("expected RootExpr::Expr, got {:?}", parsed[0].node);
        };
        let ast::Expr::For(ref for_stmt) = expr.node else {
            panic!("expected Expr::For, got {:?}", expr.node);
        };
        assert_eq!(for_stmt.block.0.len(), 1);
        let ast::Expr::IfStatement(ref if_stmt) = for_stmt.block.0[0].node else {
            panic!("expected IfStatement, got {:?}", for_stmt.block.0[0].node);
        };
        assert_eq!(if_stmt.if_node.0.len(), 1);
        assert!(matches!(if_stmt.if_node.0[0].node, ast::Expr::Continue(_)));
        let else_node = if_stmt.else_node.as_ref().expect("expected else node");
        assert_eq!(else_node.0.len(), 1);
        assert!(matches!(else_node.0[0].node, ast::Expr::Break(_)));
    }

    #[test]
    fn test_parse_for_nested() {
        let source = "for x in [1, 2] { for y in [3, 4] { continue } }";
        let parsed = parse(source).expect("should parse nested for loops");
        assert_eq!(parsed.len(), 1);
        let ast::RootExpr::Expr(ref expr) = parsed[0].node else {
            panic!("expected RootExpr::Expr, got {:?}", parsed[0].node);
        };
        let ast::Expr::For(ref outer_for) = expr.node else {
            panic!("expected Expr::For, got {:?}", expr.node);
        };
        assert_eq!(outer_for.block.0.len(), 1);
        let ast::Expr::For(ref inner_for) = outer_for.block.0[0].node else {
            panic!(
                "expected inner Expr::For, got {:?}",
                outer_for.block.0[0].node
            );
        };
        assert_eq!(inner_for.block.0.len(), 1);
        assert!(matches!(inner_for.block.0[0].node, ast::Expr::Continue(_)));
    }

    #[test]
    fn test_path_field_keywords_compatibility() {
        let source = ".for = .in\n.break = .continue";
        let parsed = parse(source).expect("should parse path fields matching keywords");
        assert_eq!(parsed.len(), 2);
        assert!(!matches!(&parsed[0].node, ast::RootExpr::Error(_)));
        assert!(!matches!(&parsed[1].node, ast::RootExpr::Error(_)));
    }

    #[test]
    fn test_ast_display() {
        let pattern_single =
            ast::ForPattern::Single(ast::Node::new(Span::new(0, 1), ast::Ident::new("item")));
        assert_eq!(format!("{pattern_single}"), "item");

        let pattern_kv = ast::ForPattern::KeyValue(
            ast::Node::new(Span::new(0, 1), ast::Ident::new("k")),
            ast::Node::new(Span::new(3, 4), ast::Ident::new("v")),
        );
        assert_eq!(format!("{pattern_kv}"), "k, v");

        let brk = ast::Break;
        assert_eq!(format!("{brk}"), "break");

        let cont = ast::Continue;
        assert_eq!(format!("{cont}"), "continue");
    }

    #[test]
    fn test_parse_for_multiline_block() {
        let source = "for x in [1, 2]\n{\n  continue\n}";
        let parsed = parse(source).expect("should parse for loop with newline before block");
        assert_eq!(parsed.len(), 1);

        let source_multiple_newlines = "for k, v in { \"a\": 1 }\n\n{\n  continue\n}";
        let parsed_mult = parse(source_multiple_newlines)
            .expect("should parse for loop with multiple newlines before block");
        assert_eq!(parsed_mult.len(), 1);
    }
}
