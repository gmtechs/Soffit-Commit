use anyhow::Result;
use std::path::Path;

/// Hash a file for content-addressing (used in conflict detection)
pub fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}
