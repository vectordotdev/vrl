use prost_reflect::{DescriptorPool, MessageDescriptor};
use std::path::Path;

pub fn get_message_pool_descriptor(
    descriptor_set_path: &Path,
) -> std::result::Result<DescriptorPool, String> {
    let b = std::fs::read(descriptor_set_path).map_err(|e| {
        format!("Failed to open protobuf desc file '{descriptor_set_path:?}': {e}",)
    })?;
    DescriptorPool::decode(b.as_slice())
        .map_err(|e| format!("Failed to parse protobuf desc file '{descriptor_set_path:?}': {e}"))
}

pub fn get_message_descriptor(
    descriptor_set_path: &Path,
    message_type: &str,
) -> std::result::Result<MessageDescriptor, String> {
    let pool = get_message_pool_descriptor(descriptor_set_path)?;
    pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("The message type '{message_type}' could not be found in '{descriptor_set_path:?}'")
    })
}

pub fn get_message_descriptor_from_pool(
    pool: &DescriptorPool,
    message_type: &str,
) -> std::result::Result<MessageDescriptor, String> {
    pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("The message type '{message_type}' could not be found in the descriptor pool")
    })
}
