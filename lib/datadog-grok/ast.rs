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
