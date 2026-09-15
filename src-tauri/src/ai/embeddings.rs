/// Embedding generation and cosine similarity search (§6.5).
/// Uses the Qwen3-Embedding-0.6B model — no text generation, pure vector math.
use crate::ai::llm;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::OnceLock;

/// Embedding context size. Text is truncated to fit with room for the BOS.
pub const EMBED_CONTEXT_TOKENS: u32 = 2_048;

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

/// Rank candidates by similarity to query embedding. Returns indices sorted by score desc.
pub fn rank_by_similarity(query: &[f32], candidates: &[Vec<f32>]) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = candidates
        .iter()
        .enumerate()
        .map(|(i, emb)| (i, cosine_similarity(query, emb)))
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Scope gate threshold — similarity below this means out-of-scope.
pub const SCOPE_THRESHOLD: f32 = 0.45;

/// Generate an embedding vector for text using the loaded embedding model.
/// Qwen3-Embedding pools the LAST token of the sequence into one fixed-size
/// vector; no generation happens here, so it cannot hallucinate (AI spec §2.2).
pub fn embed_text(model_path: &Path, text: &str) -> Result<Vec<f32>> {
    use llama_cpp_2::{
        context::params::{LlamaContextParams, LlamaPoolingType},
        llama_batch::LlamaBatch,
        model::{params::LlamaModelParams, AddBos, LlamaModel},
    };

    // Serialise with chat completions — one GGUF context at a time.
    let _slot = llm::inference_slot();
    if !model_path.exists() {
        return Err(anyhow!(
            "Embedding model not found at {}",
            model_path.display()
        ));
    }
    let backend = crate::ai::shared_backend()?;

    static EMBED_MODEL: OnceLock<
        std::result::Result<(LlamaModel, std::path::PathBuf), String>,
    > = OnceLock::new();
    let (model, loaded_from) = EMBED_MODEL
        .get_or_init(|| {
            let params = LlamaModelParams::default();
            LlamaModel::load_from_file(backend, model_path, &params)
                .map(|m| (m, model_path.to_path_buf()))
                .map_err(|e| format!("Failed to load embedding model: {e}"))
        })
        .as_ref()
        .map_err(|e| anyhow!("{e}"))?;
    if loaded_from != model_path {
        return Err(anyhow!(
            "Embedding model is already loaded from {}",
            loaded_from.display()
        ));
    }

    // Bound the prompt (~4 chars/token on this tokenizer) to stay in context.
    let text: String = text.chars().take(6_000).collect();

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(EMBED_CONTEXT_TOKENS))
        .with_n_batch(EMBED_CONTEXT_TOKENS)
        .with_n_ubatch(EMBED_CONTEXT_TOKENS)
        .with_n_threads(2)
        .with_n_threads_batch(3)
        .with_embeddings(true)
        .with_pooling_type(LlamaPoolingType::Last);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| anyhow!("Failed to create embedding context: {e}"))?;

    let mut tokens = model
        .str_to_token(&text, AddBos::Always)
        .map_err(|e| anyhow!("Embedding tokenization failed: {e}"))?;
    let max_tokens = EMBED_CONTEXT_TOKENS as usize - 1;
    tokens.truncate(max_tokens);

    let mut batch = LlamaBatch::new(EMBED_CONTEXT_TOKENS as usize, 1);
    for (i, token) in tokens.iter().enumerate() {
        batch
            .add(*token, i as i32, &[0], i + 1 == tokens.len())
            .map_err(|e| anyhow!("Embedding batch add: {e}"))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| anyhow!("Embedding decode failed: {e}"))?;

    let embedding = ctx
        .embeddings_seq_ith(0)
        .map_err(|e| anyhow!("Embedding extraction failed: {e}"))?;
    Ok(embedding.to_vec())
}
