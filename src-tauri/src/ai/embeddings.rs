/// Embedding generation and cosine similarity search (§6.5).
/// Uses the Qwen3-Embedding-0.6B model — no text generation, pure vector math.
use anyhow::{anyhow, Result};
use std::path::Path;

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { return 0.0; }
    dot / (na * nb)
}

/// Rank candidates by similarity to query embedding. Returns indices sorted by score desc.
pub fn rank_by_similarity(query: &[f32], candidates: &[Vec<f32>]) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = candidates.iter()
        .enumerate()
        .map(|(i, emb)| (i, cosine_similarity(query, emb)))
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Scope gate threshold — similarity below this means out-of-scope.
pub const SCOPE_THRESHOLD: f32 = 0.45;

/// Generate an embedding vector for text using the loaded embedding model.
/// This is a placeholder; replace with actual llama-cpp-2 embedding call once model is loaded.
pub fn embed_text(model_path: &Path, text: &str) -> Result<Vec<f32>> {
    if !model_path.exists() {
        return Err(anyhow!("Embedding model not found at {}", model_path.display()));
    }
    // TODO: replace with llama-cpp-2 LlamaContext embedding inference
    // For now returns a zero vector as placeholder until model is downloaded
    let _ = text;
    Ok(vec![0.0f32; 1024])
}
