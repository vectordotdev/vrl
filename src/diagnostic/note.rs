use std::{convert::Into, fmt};

use super::Urls;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Note {
    Hint(String),
    Example(String),
    CoerceValue,
    SeeFunctionDocs(&'static str),
    SeeErrorDocs,
    SeeCodeDocs(usize),
    SeeLangDocs,
    SeeFunctionCharacteristicsDocs,
    SeeRepl,

    #[doc(hidden)]
    SeeDocs(String, String),
    #[doc(hidden)]
    Basic(String),
    #[doc(hidden)]
    UserErrorMessage(String),
}

impl Note {
    pub fn solution(title: impl Into<String>, content: Vec<impl Into<String>>) -> Vec<Self> {
        let mut notes = vec![Self::Basic(format!("try: {}", title.into()))];

        notes.push(Self::Basic(" ".to_owned()));
        for line in content {
            notes.push(Self::Basic(format!("    {}", line.into())));
        }
        notes.push(Self::Basic(" ".to_owned()));
        notes
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Note::{
            Basic, CoerceValue, Example, Hint, SeeCodeDocs, SeeDocs, SeeErrorDocs,
            SeeFunctionCharacteristicsDocs, SeeFunctionDocs, SeeLangDocs, SeeRepl,
            UserErrorMessage,
        };

        match self {
            Hint(hint) => {
                write!(f, "hint: {hint}")
            }
            Example(example) => {
                write!(f, "example: {example}")
            }
            CoerceValue => {
                Hint("coerce the value to the required type using a coercion function".to_owned())
                    .fmt(f)
            }
            SeeFunctionDocs(ident) => {
                let url = Urls::func_docs(ident);
                SeeDocs("function".to_owned(), url).fmt(f)
            }
            SeeErrorDocs => {
                let url = Urls::error_handling_url();
                SeeDocs("error handling".to_owned(), url).fmt(f)
            }
            SeeLangDocs => {
                let url = Urls::vrl_root_url();

                write!(f, "see language documentation at {url}")
            }
            SeeFunctionCharacteristicsDocs => {
                let url = Urls::func_characteristics();
                write!(f, "see functions characteristics documentation at {url}")
            }
            SeeRepl => {
                let url = Urls::example_docs();

                write!(f, "try your code in the VRL REPL, learn more at {url}")
            }
            SeeCodeDocs(code) => {
                let url = Urls::error_code_url(*code);
                write!(f, "learn more about error code {code} at {url}")
            }
            SeeDocs(kind, url) => {
                write!(f, "see documentation about {kind} at {url}")
            }
            Basic(string) => write!(f, "{string}"),
            UserErrorMessage(message) => write!(f, "{message}"),
        }
    }
}
