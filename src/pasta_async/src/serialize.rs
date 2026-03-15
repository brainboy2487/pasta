use serde_json::Value;

// Placeholder serializer helpers. Interpreter must implement Continuation::serialize/deserialize.
pub fn compute_checksum(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
