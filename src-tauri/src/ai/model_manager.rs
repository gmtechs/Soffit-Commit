/// Download, verify, and manage the two GGUF model files.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const CHAT_MODEL_URL: &str =
    "https://huggingface.co/bartowski/Qwen_Qwen3-0.6B-GGUF/resolve/main/Qwen_Qwen3-0.6B-Q4_K_M.gguf";
pub const CHAT_MODEL_FILENAME: &str = "Qwen3-0.6B-Q4_K_M.gguf";
// Optional upstream pin. Empty means "trust the sidecar checksum": the first
// successful download records the file's SHA-256 next to it and every later
// launch re-verifies the file against that record.
pub const CHAT_MODEL_SHA256: &str = "";

pub const EMBED_MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf";
pub const EMBED_MODEL_FILENAME: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";
pub const EMBED_MODEL_SHA256: &str = "";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    NotDownloaded,
    Downloading { bytes_done: u64, bytes_total: u64 },
    Ready,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub chat_model: ModelStatus,
    pub embed_model: ModelStatus,
    pub chat_size_bytes: u64,
    pub embed_size_bytes: u64,
}

pub struct ModelManager {
    pub models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let models_dir = app_data_dir.join("models");
        std::fs::create_dir_all(&models_dir).ok();
        Self { models_dir }
    }

    pub fn chat_model_path(&self) -> PathBuf {
        self.models_dir.join(CHAT_MODEL_FILENAME)
    }

    pub fn embed_model_path(&self) -> PathBuf {
        self.models_dir.join(EMBED_MODEL_FILENAME)
    }

    pub fn chat_ready(&self) -> bool {
        is_valid_gguf(&self.chat_model_path())
    }

    pub fn embed_ready(&self) -> bool {
        is_valid_gguf(&self.embed_model_path())
    }

    pub fn status(&self) -> AiStatus {
        let chat_size = self
            .chat_model_path()
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        let embed_size = self
            .embed_model_path()
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        AiStatus {
            chat_model: model_status(&self.chat_model_path()),
            embed_model: model_status(&self.embed_model_path()),
            chat_size_bytes: chat_size,
            embed_size_bytes: embed_size,
        }
    }

    /// Download a model file with progress callback. The stream is hashed
    /// incrementally; the accepted hash is recorded in a `<file>.sha256`
    /// sidecar and re-verified on every subsequent launch.
    pub async fn download(
        &self,
        url: &str,
        filename: &str,
        expected_sha256: &str,
        progress_cb: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<()> {
        let dest = self.models_dir.join(filename);
        let tmp = self.models_dir.join(format!("{filename}.part"));
        let sidecar = self.sidecar_path(filename);
        let previous = std::fs::read_to_string(&sidecar)
            .ok()
            .map(|s| s.trim().to_string());

        let response = reqwest::get(url)
            .await
            .map_err(|e| anyhow!("Download failed: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow!("Model server returned an error: {e}"))?;
        let total = response.content_length().unwrap_or(0);

        let mut file = tokio::fs::File::create(&tmp)
            .await
            .map_err(|e| anyhow!("Cannot create temp file: {e}"))?;

        let mut downloaded: u64 = 0;
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow!("Download stream error: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| anyhow!("Write error: {e}"))?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;
            progress_cb(downloaded, total);
        }
        file.flush().await.ok();
        drop(file);

        if !is_valid_gguf(&tmp) {
            std::fs::remove_file(&tmp).ok();
            return Err(anyhow!(
                "Downloaded file for {filename} is not a valid GGUF model"
            ));
        }

        let hash = format!("{:x}", hasher.finalize());

        // 1) A pinned upstream hash (when provided) must match exactly.
        if !expected_sha256.is_empty()
            && !expected_sha256.starts_with("00000000")
            && hash != expected_sha256
        {
            std::fs::remove_file(&tmp).ok();
            return Err(anyhow!(
                "Checksum mismatch for {filename}: expected {expected_sha256}, got {hash}"
            ));
        }
        // 2) A re-download must match the previously accepted copy — a silent
        //    upstream swap or truncated mirror is rejected here.
        if let Some(prev) = previous {
            if prev != hash {
                std::fs::remove_file(&tmp).ok();
                return Err(anyhow!(
                    "Checksum changed for {filename} since the last download ({prev} → {hash}); \
                     refusing the new copy"
                ));
            }
        }
        // 3) Record the accepted hash for future launch-time verification.
        std::fs::write(&sidecar, &hash)
            .map_err(|e| anyhow!("Cannot write checksum sidecar: {e}"))?;

        std::fs::rename(&tmp, &dest).map_err(|e| anyhow!("Cannot move file: {e}"))?;
        Ok(())
    }

    /// Remove both model files (and their checksum sidecars) to free disk space.
    pub fn remove_models(&self) -> Result<()> {
        for filename in [CHAT_MODEL_FILENAME, EMBED_MODEL_FILENAME] {
            for path in [
                self.models_dir.join(filename),
                self.sidecar_path(filename),
                self.models_dir.join(format!("{filename}.part")),
            ] {
                if path.exists() {
                    std::fs::remove_file(&path)?;
                }
            }
        }
        Ok(())
    }

    fn sidecar_path(&self, filename: &str) -> PathBuf {
        self.models_dir.join(format!("{filename}.sha256"))
    }
}

/// A model must start with the GGUF file signature and be larger than a header.
/// This prevents small error documents from being considered downloaded models.
fn is_valid_gguf(path: &Path) -> bool {
    const MIN_MODEL_BYTES: u64 = 1_048_576;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() < MIN_MODEL_BYTES {
        return false;
    }

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0_u8; 4];
    use std::io::Read;
    file.read_exact(&mut magic).is_ok() && magic == *b"GGUF"
}

fn model_status(path: &Path) -> ModelStatus {
    if !path.exists() {
        ModelStatus::NotDownloaded
    } else if !is_valid_gguf(path) {
        ModelStatus::Error("Downloaded file is invalid; download it again.".to_string())
    } else if let Err(msg) = verify_against_sidecar(path) {
        ModelStatus::Error(msg)
    } else {
        ModelStatus::Ready
    }
}

/// Re-verify a downloaded model against the checksum sidecar recorded when it
/// was accepted. A missing sidecar (downloaded by an older build) is fine; a
/// mismatched one means the file was corrupted or tampered with on disk.
fn verify_against_sidecar(path: &Path) -> Result<(), String> {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(".sha256");
    let sidecar = PathBuf::from(sidecar);
    let Some(expected) = std::fs::read_to_string(&sidecar)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let actual = sha256_file(path).map_err(|e| format!("Cannot hash model file: {e}"))?;
    if actual != expected {
        return Err(format!(
            "Integrity check failed for {} (expected {expected}, got {actual}); download it again.",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("model")
        ));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::is_valid_gguf;
    use std::io::Write;

    #[test]
    fn rejects_non_gguf_downloads() {
        let path = std::env::temp_dir().join(format!("soffit-invalid-{}.gguf", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"Entry not found").unwrap();
        drop(file);

        assert!(!is_valid_gguf(&path));
        std::fs::remove_file(path).unwrap();
    }
}
