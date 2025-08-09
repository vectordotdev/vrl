mod descriptor;
mod encode;
mod parse;

pub use descriptor::get_message_descriptor;
pub use descriptor::get_message_descriptor_from_pool;
pub use descriptor::get_message_pool_descriptor;

pub use encode::encode_message;
pub(crate) use encode::encode_proto;

pub(crate) use parse::parse_proto;
pub use parse::proto_to_value;
