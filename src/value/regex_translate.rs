//! Lower Rust's validated HIR to PCRE2 instead of asking PCRE2 to reinterpret
//! Rust character classes, flags, capture names, or newline/word assertions.
use regex_syntax::hir::{Class, Hir, HirKind, Look};
use std::{fmt::Write, sync::OnceLock};

fn unicode_class(class: &regex_syntax::hir::ClassUnicode) -> String {
    if class.iter().next().is_none() {
        return "(?!)".into();
    }
    let mut output = String::from("[");
    for range in class.iter() {
        write!(output, "\\x{{{:x}}}", u32::from(range.start())).expect("write to string");
        if range.start() != range.end() {
            write!(output, "-\\x{{{:x}}}", u32::from(range.end())).expect("write to string");
        }
    }
    output.push(']');
    output
}

fn word(unicode: bool) -> &'static str {
    static WORD: OnceLock<String> = OnceLock::new();
    if !unicode {
        return "[A-Za-z0-9_]";
    }
    WORD.get_or_init(|| {
        let hir = regex_syntax::Parser::new()
            .parse(r"\w")
            .expect("built-in word class");
        let HirKind::Class(Class::Unicode(class)) = hir.kind() else {
            unreachable!("word is a Unicode class")
        };
        unicode_class(class)
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the exhaustive HIR lowering in one match."
)]
fn emit(hir: &Hir, out: &mut String) -> Result<(), String> {
    match hir.kind() {
        HirKind::Empty => out.push_str("(?:)"),
        HirKind::Literal(literal) => {
            let text = std::str::from_utf8(&literal.0)
                .map_err(|_| "PCRE2 JIT does not support a partial UTF-8 byte literal")?;
            for ch in text.chars() {
                write!(out, "\\x{{{:x}}}", u32::from(ch)).expect("write to string");
            }
        }
        HirKind::Class(Class::Unicode(class)) => out.push_str(&unicode_class(class)),
        HirKind::Class(Class::Bytes(class)) => {
            if class.iter().next().is_none() {
                out.push_str("(?!)");
                return Ok(());
            }
            out.push('[');
            for range in class.iter() {
                if range.end() >= 128 {
                    return Err("PCRE2 JIT does not support non-ASCII byte-mode classes inside UTF-8 patterns".into());
                }
                write!(out, "\\x{{{:x}}}", range.start()).expect("write to string");
                if range.start() != range.end() {
                    write!(out, "-\\x{{{:x}}}", range.end()).expect("write to string");
                }
            }
            out.push(']');
        }
        HirKind::Look(look) => match look {
            Look::Start => out.push_str(r"\A"),
            Look::End => out.push_str(r"\z"),
            Look::StartLF => out.push_str(r"(?:\A|(?<=\n))"),
            Look::EndLF => out.push_str(r"(?:\z|(?=\n))"),
            Look::StartCRLF => out.push_str(r"(?:\A|(?<=\n)|(?<=\r)(?!\n))"),
            Look::EndCRLF => out.push_str(r"(?:\z|(?=\r)|(?<!\r)(?=\n))"),
            look => {
                let unicode = matches!(
                    look,
                    Look::WordUnicode
                        | Look::WordUnicodeNegate
                        | Look::WordStartUnicode
                        | Look::WordEndUnicode
                        | Look::WordStartHalfUnicode
                        | Look::WordEndHalfUnicode
                );
                let w = word(unicode);
                match look {
                    Look::WordAscii | Look::WordUnicode => {
                        write!(out, "(?:(?<={w})(?!{w})|(?<!{w})(?={w}))")
                    }
                    Look::WordAsciiNegate | Look::WordUnicodeNegate => {
                        write!(out, "(?:(?<={w})(?={w})|(?<!{w})(?!{w}))")
                    }
                    Look::WordStartAscii | Look::WordStartUnicode => write!(out, "(?<!{w})(?={w})"),
                    Look::WordEndAscii | Look::WordEndUnicode => write!(out, "(?<={w})(?!{w})"),
                    Look::WordStartHalfAscii | Look::WordStartHalfUnicode => {
                        write!(out, "(?<!{w})")
                    }
                    Look::WordEndHalfAscii | Look::WordEndHalfUnicode => write!(out, "(?!{w})"),
                    _ => unreachable!("other assertions handled above"),
                }
                .expect("write to string");
            }
        },
        HirKind::Repetition(rep) => {
            out.push_str("(?:");
            emit(&rep.sub, out)?;
            out.push(')');
            match (rep.min, rep.max) {
                (0, None) => out.push('*'),
                (1, None) => out.push('+'),
                (0, Some(1)) => out.push('?'),
                (min, Some(max)) if min == max => {
                    write!(out, "{{{min}}}").expect("write to string");
                }
                (min, Some(max)) => write!(out, "{{{min},{max}}}").expect("write to string"),
                (min, None) => write!(out, "{{{min},}}").expect("write to string"),
            }
            if !rep.greedy {
                out.push('?');
            }
        }
        HirKind::Capture(capture) => {
            out.push('(');
            emit(&capture.sub, out)?;
            out.push(')');
        }
        HirKind::Concat(items) => {
            for item in items {
                emit(item, out)?;
            }
        }
        HirKind::Alternation(items) => {
            out.push_str("(?:");
            for (i, item) in items.iter().enumerate() {
                if i != 0 {
                    out.push('|');
                }
                emit(item, out)?;
            }
            out.push(')');
        }
    }
    Ok(())
}

pub(crate) fn translate(pattern: &str) -> Result<String, String> {
    let hir = regex_syntax::Parser::new()
        .parse(pattern)
        .map_err(|e| e.to_string())?;
    let mut result = String::new();
    emit(&hir, &mut result)?;
    Ok(result)
}
