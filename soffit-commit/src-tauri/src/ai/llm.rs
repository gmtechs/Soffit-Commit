/// llama-cpp-2 wrapper: load model, run chat completion, stream tokens.
use anyhow::{anyhow, Result};
use std::path::Path;
use crate::ai::identity_guard;

pub fn complete(
    model_path: &Path,
    system_prompt: &str,
    user_message: &str,
    _grammar_gbnf: Option<&str>,
    on_token: impl Fn(&str) -> bool,
) -> Result<String> {
    if !model_path.exists() {
        return Err(anyhow!("Chat model not found at {}", model_path.display()));
    }

    use llama_cpp_2::{
        context::params::LlamaContextParams,
        llama_backend::LlamaBackend,
        llama_batch::LlamaBatch,
        model::{params::LlamaModelParams, AddBos, LlamaModel},
        token::data_array::LlamaTokenDataArray,
    };

    let backend = LlamaBackend::init()?;
    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
        .map_err(|e| anyhow!("Failed to load chat model: {e}"))?;

    // Qwen3 supports a substantially larger context than the old 2K default.
    // This prevents normal documents from failing before inference begins.
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(8192));
    let mut ctx = model.new_context(&backend, ctx_params)
        .map_err(|e| anyhow!("Failed to create context: {e}"))?;

    let prompt = format!(
        "<|im_start|>system\n{system_prompt}<|im_end|>\n\
         <|im_start|>user\n{user_message}<|im_end|>\n\
         <|im_start|>assistant\n"
    );

    let mut tokens = model.str_to_token(&prompt, AddBos::Always)
        .map_err(|e| anyhow!("Tokenization failed: {e}"))?;

    let n_ctx = ctx.n_ctx() as usize;
    // Keep generation room as well as prompt room. Never expose a context-size
    // error to callers: retain the prompt identity/header and its most recent
    // relevant content (including the user's question) when compression is
    // necessary. Document chat already ranks passages before this safety net.
    if tokens.len() >= n_ctx.saturating_sub(512) {
        let limit = n_ctx.saturating_sub(512);
        let prefix_len = limit.min(512).min(tokens.len());
        let suffix_len = limit.saturating_sub(prefix_len).min(tokens.len().saturating_sub(prefix_len));
        let mut compressed = tokens[..prefix_len].to_vec();
        compressed.extend_from_slice(&tokens[tokens.len() - suffix_len..]);
        tokens = compressed;
    }

    // llama.cpp batches have a fixed capacity. Long document prompts must be
    // decoded in chunks rather than trying to put every prompt token in one
    // 512-token batch.
    const BATCH_SIZE: usize = 512;
    let mut batch = LlamaBatch::new(BATCH_SIZE, 1);
    let last_idx = (tokens.len() - 1) as i32;
    for (chunk_index, token_chunk) in tokens.chunks(BATCH_SIZE).enumerate() {
        batch.clear();
        let offset = chunk_index * BATCH_SIZE;
        for (index, token) in token_chunk.iter().enumerate() {
            let position = offset + index;
            batch.add(*token, position as i32, &[0], position as i32 == last_idx)
                .map_err(|e| anyhow!("Batch add: {e}"))?;
        }
        ctx.decode(&mut batch).map_err(|e| anyhow!("Decode: {e}"))?;
    }

    let mut output = String::new();
    let mut n_cur = tokens.len() as i32;
    let mut generated = 0i32;
    let n_predict = 512i32;
    let eos = model.token_eos();
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    loop {
        let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
        let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
        let new_token = candidates_p.sample_token_greedy();

        if new_token == eos || generated >= n_predict || n_cur >= n_ctx as i32 { break; }

        let piece = model.token_to_piece(new_token, &mut decoder, false, None)
            .map_err(|e| anyhow!("token_to_piece: {e}"))?;

        output.push_str(&piece);
        if !on_token(&piece) { break; }
        if identity_guard::buffer_has_leak(&output) {
            return Ok(identity_guard::canned().to_string());
        }

        batch.clear();
        batch.add(new_token, n_cur, &[0], true)
            .map_err(|e| anyhow!("Batch add: {e}"))?;
        ctx.decode(&mut batch).map_err(|e| anyhow!("Decode: {e}"))?;
        n_cur += 1;
        generated += 1;
    }

    Ok(identity_guard::guard(&output))
}
