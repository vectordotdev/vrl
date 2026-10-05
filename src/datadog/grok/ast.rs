use crate::path::OwnedValuePath;
use crate::value::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct GrokPattern {
    pub match_fn: Function,
    pub destination: Option<Destination>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Destination {
    pub path: OwnedValuePath,
    pub filter_fn: Option<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub args: Option<Vec<FunctionArgument>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FunctionArgument {
    Function(Function),
    Arg(Value),
}

impl FunctionArgument {
    pub fn as_bytes(&self) -> Option<&bytes::Bytes> {
        match self {
            Self::Arg(v) => v.as_bytes(),
            Self::Function(_) => None,
        }
    }

    pub fn to_utf8_lossy(&self) -> Option<String> {
        match self {
            Self::Arg(v) => v.as_str().map(std::borrow::Cow::into_owned),
            Self::Function(_) => None,
        }
    }
}
