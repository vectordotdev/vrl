use prost_reflect::{DescriptorPool, MessageDescriptor};
use std::path::Path;

pub fn get_message_descriptor(
    descriptor_set_path: &Path,
    message_type: &str,
) -> std::result::Result<MessageDescriptor, String> {
    let b = std::fs::read(descriptor_set_path).map_err(|e| {
        format!("Failed to open protobuf desc file '{descriptor_set_path:?}': {e}",)
    })?;
    let pool = DescriptorPool::decode(b.as_slice()).map_err(|e| {
        format!("Failed to parse protobuf desc file '{descriptor_set_path:?}': {e}")
    })?;
    pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("The message type '{message_type}' could not be found in '{descriptor_set_path:?}'")
    })
}
