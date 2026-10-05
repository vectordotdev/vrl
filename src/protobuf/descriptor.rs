use prost_reflect::{DescriptorPool, MessageDescriptor};
use std::path::Path;

/// Loads a descriptor set and resolves a message type from it.
///
/// # Errors
///
/// Returns an error if the descriptor set cannot be read or decoded, or if it
/// does not contain `message_type`.
pub fn get_message_descriptor(
    descriptor_set_path: &Path,
    message_type: &str,
) -> Result<MessageDescriptor, String> {
    let path = descriptor_set_path.display();
    let bytes = std::fs::read(descriptor_set_path)
        .map_err(|e| format!("Failed to open protobuf desc file '{path}': {e}"))?;
    let pool = DescriptorPool::decode(bytes.as_slice())
        .map_err(|e| format!("Failed to parse protobuf desc file '{path}': {e}"))?;
    resolve_message_descriptor(
        &pool,
        message_type,
        &format!("'{}'", descriptor_set_path.display()),
    )
}

/// Resolves a message type from an encoded descriptor set.
///
/// # Errors
///
/// Returns an error if the descriptor set cannot be decoded or does not
/// contain `message_type`.
pub fn get_message_descriptor_from_bytes(
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<MessageDescriptor, String> {
    let pool = DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| format!("Failed to parse protobuf descriptor bytes: {e}"))?;
    resolve_message_descriptor(&pool, message_type, "the provided descriptor bytes")
}

fn resolve_message_descriptor(
    descriptor_pool: &DescriptorPool,
    message_type: &str,
    context: &str,
) -> Result<MessageDescriptor, String> {
    descriptor_pool
        .get_message_by_name(message_type)
        .ok_or_else(|| format!("The message type '{message_type}' could not be found in {context}"))
}
