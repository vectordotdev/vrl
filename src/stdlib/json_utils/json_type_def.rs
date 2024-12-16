use crate::prelude::{Collection, TypeDef};
use crate::value::Kind;

pub(crate) fn json_inner_kind() -> Kind {
    Kind::null()
        | Kind::bytes()
        | Kind::integer()
        | Kind::float()
        | Kind::boolean()
        | Kind::array(Collection::any())
        | Kind::object(Collection::any())
}

pub(crate) fn json_type_def() -> TypeDef {
    TypeDef::bytes()
        .fallible()
        .or_boolean()
        .or_integer()
        .or_float()
        .add_null()
        .or_null()
        .or_array(Collection::from_unknown(json_inner_kind()))
        .or_object(Collection::from_unknown(json_inner_kind()))
}
