use anyhow::Result;
use iroh::SecretKey;
use std::path::Path;

pub fn load_or_create(path: &Path) -> Result<SecretKey> {
    if path.exists() {
        let bytes = std::fs::read(path)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid secret key length on disk"))?;
        Ok(SecretKey::from_bytes(&arr))
    } else {
        // iroh 1.x: SecretKey::generate() takes no arguments
        let key = SecretKey::generate();
        std::fs::write(path, key.to_bytes())?;
        Ok(key)
    }
}
