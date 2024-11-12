//!
//! This contains all the functionality for the JIT (Just In Time) features of the lookup path.
//! This allows parsing on the fly as it's needed.
//!
//! This contains a hand-written state machine to maximize performance of the parser.
//! This is important since a lot of parsing happens at runtime. Eventually Vector
//! should pre-compile all paths. Once that happens it might make sense to re-write in something
//! more readable.

use std::borrow::Cow;
use std::str::CharIndices;

use super::{BorrowedSegment, ValuePath};

#[derive(Clone)]
pub struct JitValuePath<'a> {
    path: &'a str,
}

impl JitValuePath<'_> {
    pub fn new(path: &str) -> JitValuePath {
        JitValuePath { path }
    }
}

#[derive(Clone)]
pub struct JitValuePathIter<'a> {
    path: &'a str,
    chars: CharIndices<'a>,
    state: JitState,
    escape_buffer: String,
}

impl<'a> JitValuePathIter<'a> {
    pub fn new(path: &'a str) -> Self {
        Self {
            chars: path.char_indices(),
            path,
            state: JitState::Start,
            escape_buffer: String::new(),
        }
    }
}

impl<'a> ValuePath<'a> for JitValuePath<'a> {
    type Iter = JitValuePathIter<'a>;

    fn segment_iter(&self) -> Self::Iter {
        JitValuePathIter::new(self.path)
    }
}

#[derive(Clone)]
enum JitState {
    EventRoot,
    Start,
    Continue,
    Dot,
    IndexStart,
    NegativeIndex { value: isize },
    Index { value: isize },
    Field { start: usize },
    Quote { start: usize },
    EscapedQuote,
    End,
}

impl<'a> Iterator for JitValuePathIter<'a> {
    type Item = BorrowedSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.chars.next() {
                None => {
                    let result = match self.state {
                        JitState::Start
                        | JitState::IndexStart
                        | JitState::Index { .. }
                        | JitState::NegativeIndex { .. }
                        | JitState::Quote { .. }
                        | JitState::EscapedQuote { .. }
                        | JitState::Dot => Some(BorrowedSegment::Invalid),

                        JitState::Continue | JitState::EventRoot | JitState::End => None,

                        JitState::Field { start } => {
                            Some(BorrowedSegment::Field(Cow::Borrowed(&self.path[start..])))
                        }
                    };
                    self.state = JitState::End;
                    return result;
                }
                Some((index, c)) => {
                    let (result, state) = match self.state {
                        JitState::Start => match c {
                            '.' => (None, JitState::EventRoot),
                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@' | '-' => {
                                (None, JitState::Field { start: index })
                            }
                            '[' => (None, JitState::IndexStart),
                            '\"' => (None, JitState::Quote { start: index + 1 }),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::Continue => match c {
                            '.' => (None, JitState::Dot),
                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@' | '-' => {
                                (None, JitState::Field { start: index })
                            }
                            '[' => (None, JitState::IndexStart),
                            '\"' => (None, JitState::Quote { start: index + 1 }),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::EventRoot => match c {
                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@' | '-' => {
                                (None, JitState::Field { start: index })
                            }
                            '[' => (None, JitState::IndexStart),
                            '\"' => (None, JitState::Quote { start: index + 1 }),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::Dot => match c {
                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@' | '-' => {
                                (None, JitState::Field { start: index })
                            }
                            '\"' => (None, JitState::Quote { start: index + 1 }),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::Field { start } => match c {
                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@' | '-' => {
                                (None, JitState::Field { start })
                            }
                            '.' => (
                                Some(Some(BorrowedSegment::Field(Cow::Borrowed(
                                    &self.path[start..index],
                                )))),
                                JitState::Dot,
                            ),
                            '[' => (
                                Some(Some(BorrowedSegment::Field(Cow::Borrowed(
                                    &self.path[start..index],
                                )))),
                                JitState::IndexStart,
                            ),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::Quote { start } => match c {
                            '\"' => (
                                Some(Some(BorrowedSegment::Field(Cow::Borrowed(
                                    &self.path[start..index],
                                )))),
                                JitState::Continue,
                            ),
                            '\\' => {
                                // Character escaping requires copying chars to a new String.
                                // State is reverted back to the start of the quote to start over
                                // with the copy method (which is slower)
                                self.path = &self.path[start..];
                                self.chars = self.path.char_indices();
                                (None, JitState::EscapedQuote)
                            }
                            _ => (None, JitState::Quote { start }),
                        },
                        JitState::EscapedQuote => match c {
                            '\"' => (
                                (Some(Some(BorrowedSegment::Field(
                                    std::mem::take(&mut self.escape_buffer).into(),
                                )))),
                                JitState::Continue,
                            ),
                            '\\' => match self.chars.next() {
                                Some((_, c)) => match c {
                                    '\\' | '\"' => {
                                        self.escape_buffer.push(c);
                                        (None, JitState::EscapedQuote)
                                    }
                                    _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                                },
                                None => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                            },
                            _ => {
                                self.escape_buffer.push(c);
                                (None, JitState::EscapedQuote)
                            }
                        },
                        JitState::IndexStart => match c {
                            '0'..='9' => (
                                None,
                                JitState::Index {
                                    value: c as isize - '0' as isize,
                                },
                            ),
                            '-' => (None, JitState::NegativeIndex { value: 0 }),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::Index { value } => match c {
                            '0'..='9' => {
                                let new_digit = c as isize - '0' as isize;
                                (
                                    None,
                                    JitState::Index {
                                        value: value * 10 + new_digit,
                                    },
                                )
                            }
                            ']' => (
                                Some(Some(BorrowedSegment::Index(value))),
                                JitState::Continue,
                            ),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::NegativeIndex { value } => match c {
                            '0'..='9' => {
                                let new_digit = c as isize - '0' as isize;
                                (
                                    None,
                                    JitState::NegativeIndex {
                                        value: value * 10 - new_digit,
                                    },
                                )
                            }
                            ']' => (
                                Some(Some(BorrowedSegment::Index(value))),
                                JitState::Continue,
                            ),
                            _ => (Some(Some(BorrowedSegment::Invalid)), JitState::End),
                        },
                        JitState::End => (Some(None), JitState::End),
                    };
                    self.state = state;
                    if let Some(result) = result {
                        return result;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::{BorrowedSegment, ValuePath};

    #[test]
    fn parsing() {
        let test_cases = vec![
            ("", vec![BorrowedSegment::Invalid]),
            (".", vec![]),
            ("]", vec![BorrowedSegment::Invalid]),
            ("]foo", vec![BorrowedSegment::Invalid]),
            ("..", vec![BorrowedSegment::Invalid]),
            ("...", vec![BorrowedSegment::Invalid]),
            ("f", vec![BorrowedSegment::Field("f".into())]),
            (".f", vec![BorrowedSegment::Field("f".into())]),
            (".[", vec![BorrowedSegment::Invalid]),
            (
                "f.",
                vec![BorrowedSegment::Field("f".into()), BorrowedSegment::Invalid],
            ),
            ("foo", vec![BorrowedSegment::Field("foo".into())]),
            (
                "ec2.metadata.\"availability-zone\"",
                vec![
                    BorrowedSegment::Field("ec2".into()),
                    BorrowedSegment::Field("metadata".into()),
                    BorrowedSegment::Field("availability-zone".into()),
                ],
            ),
            (".foo", vec![BorrowedSegment::Field("foo".into())]),
            (".a-b", vec![BorrowedSegment::Field("a-b".into())]),
            (".a-b-", vec![BorrowedSegment::Field("a-b-".into())]),
            (".-a-b", vec![BorrowedSegment::Field("-a-b".into())]),
            (
                ".@timestamp",
                vec![BorrowedSegment::Field("@timestamp".into())],
            ),
            (
                "foo[",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Invalid,
                ],
            ),
            ("foo$", vec![BorrowedSegment::Invalid]),
            (
                "\"$peci@l chars\"",
                vec![BorrowedSegment::Field("$peci@l chars".into())],
            ),
            (
                ".foo.foo bar",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Invalid,
                ],
            ),
            (
                ".foo.\"foo bar\".bar",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Field("foo bar".into()),
                    BorrowedSegment::Field("bar".into()),
                ],
            ),
            ("[1]", vec![BorrowedSegment::Index(1)]),
            ("[42]", vec![BorrowedSegment::Index(42)]),
            (".[42]", vec![BorrowedSegment::Index(42)]),
            (
                "[42].foo",
                vec![
                    BorrowedSegment::Index(42),
                    BorrowedSegment::Field("foo".into()),
                ],
            ),
            (
                "foo.[42]",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Invalid,
                ],
            ),
            (
                "foo..bar",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Invalid,
                ],
            ),
            (
                "[42]foo",
                vec![
                    BorrowedSegment::Index(42),
                    BorrowedSegment::Field("foo".into()),
                ],
            ),
            ("[-1]", vec![BorrowedSegment::Index(-1)]),
            ("[-42]", vec![BorrowedSegment::Index(-42)]),
            (".[-42]", vec![BorrowedSegment::Index(-42)]),
            (
                "[-42].foo",
                vec![
                    BorrowedSegment::Index(-42),
                    BorrowedSegment::Field("foo".into()),
                ],
            ),
            (
                "[-42]foo",
                vec![
                    BorrowedSegment::Index(-42),
                    BorrowedSegment::Field("foo".into()),
                ],
            ),
            (
                ".\"[42]. {}-_\"",
                vec![BorrowedSegment::Field("[42]. {}-_".into())],
            ),
            ("\"a\\\"a\"", vec![BorrowedSegment::Field("a\"a".into())]),
            (".\"a\\\"a\"", vec![BorrowedSegment::Field("a\"a".into())]),
            (
                ".foo.\"a\\\"a\".\"b\\\\b\".bar",
                vec![
                    BorrowedSegment::Field("foo".into()),
                    BorrowedSegment::Field("a\"a".into()),
                    BorrowedSegment::Field("b\\b".into()),
                    BorrowedSegment::Field("bar".into()),
                ],
            ),
            (r#"."🤖""#, vec![BorrowedSegment::Field("🤖".into())]),
            ("(a)", vec![BorrowedSegment::Invalid]),
        ];

        for (path, expected) in test_cases {
            if !ValuePath::eq(&path, &expected) {
                panic!(
                    "Not equal. Input={:?}\nExpected: {:?}\nActual: {:?}",
                    path,
                    (&expected).segment_iter().collect::<Vec<_>>(),
                    path.segment_iter().collect::<Vec<_>>()
                );
            }
        }
    }
}
