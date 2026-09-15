/// Soffit AI — offline, in-process AI layer powered by llama-cpp-2.
/// All six features from the spec are routed through this module.
pub mod embeddings;
pub mod grammar;
pub mod identity_guard;
pub mod llm;
pub mod model_manager;
pub mod prompts;
pub mod scope_guard;

use std::sync::OnceLock;

/// One llama.cpp backend for the whole process — chat completions and
/// embeddings share it (llama.cpp expects a single backend init).
static BACKEND: OnceLock<std::result::Result<llama_cpp_2::llama_backend::LlamaBackend, String>> =
    OnceLock::new();

pub(crate) fn shared_backend(
) -> anyhow::Result<&'static llama_cpp_2::llama_backend::LlamaBackend> {
    BACKEND
        .get_or_init(|| {
            let mut backend = llama_cpp_2::llama_backend::LlamaBackend::init()
                .map_err(|e| e.to_string())?;
            // llama.cpp emits a verbose tensor-by-tensor loading trace. Keep
            // the terminal useful and retain the callers' concise logs.
            backend.void_logs();
            Ok(backend)
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to initialise llama.cpp backend: {e}"))
}
