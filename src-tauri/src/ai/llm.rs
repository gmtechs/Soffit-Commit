use crate::ai::identity_guard;
/// llama-cpp-2 wrapper: load model, run chat completion, stream tokens.
use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// The document-chat budget is deliberately small. Retrieval selects relevant
/// passages before inference, so allocating an 8K context merely wastes RAM.
/// On Qwen3-0.6B this replaces the old ~896 MiB KV cache with ~336 MiB.
pub const CHAT_CONTEXT_TOKENS: u32 = 3_072;
pub const CHAT_BATCH_TOKENS: u32 = 512;
pub const CHAT_UBATCH_TOKENS: u32 = 256;
pub const CHAT_GENERATION_THREADS: i32 = 2;
pub const CHAT_PROMPT_THREADS: i32 = 3;
pub const CHAT_MAX_GENERATED_TOKENS: i32 = 128;

/// llama.cpp contexts are not safely parallel for this app's memory budget.
/// Serialize all completions until the dedicated long-lived worker lands.
static INFERENCE_GATE: OnceLock<Mutex<()>> = OnceLock::new();

/// Reserve the inference slot. Embeddings share it so a semantic search can
/// never run concurrently with a completion and blow the memory budget.
pub(crate) fn inference_slot() -> std::sync::MutexGuard<'static, ()> {
    INFERENCE_GATE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// Model weights are immutable and LlamaModel is explicitly Send + Sync in the
// binding. Retaining this object avoids a full GGUF load and CPU repack for
// every question. Contexts remain request-scoped and serialised because they
// hold mutable KV state. The backend is process-wide (see ai::shared_backend).
static MODEL: OnceLock<std::result::Result<llama_cpp_2::model::LlamaModel, String>> =
    OnceLock::new();

#[derive(Clone, Copy)]
enum CompletionMode {
    Default,
    DirectDocumentAnswer,
}

pub fn complete(
    model_path: &Path,
    system_prompt: &str,
    user_message: &str,
    grammar_gbnf: Option<&str>,
    on_token: impl Fn(&str) -> bool,
) -> Result<String> {
    complete_with_mode(
        model_path,
        system_prompt,
        user_message,
        CompletionMode::Default,
        grammar_gbnf,
        on_token,
    )
}

/// Document retrieval is an extraction task, not an open-ended reasoning task.
/// Qwen3 honors an already-closed think block and begins with the user-facing
/// answer, avoiding unnecessary reasoning tokens on CPU.
pub fn complete_document(
    model_path: &Path,
    system_prompt: &str,
    user_message: &str,
    on_token: impl Fn(&str) -> bool,
) -> Result<String> {
    complete_with_mode(
        model_path,
        system_prompt,
        user_message,
        CompletionMode::DirectDocumentAnswer,
        None,
        on_token,
    )
}

fn complete_with_mode(
    model_path: &Path,
    system_prompt: &str,
    user_message: &str,
    mode: CompletionMode,
    grammar_gbnf: Option<&str>,
    on_token: impl Fn(&str) -> bool,
) -> Result<String> {
    let _inference_slot = INFERENCE_GATE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow!("The local inference worker is unavailable"))?;
    let started = Instant::now();
    if !model_path.exists() {
        return Err(anyhow!("Chat model not found at {}", model_path.display()));
    }

    use llama_cpp_2::{
        context::params::LlamaContextParams,
        llama_batch::LlamaBatch,
        model::{params::LlamaModelParams, AddBos, LlamaModel},
        token::data_array::LlamaTokenDataArray,
    };

    let backend = crate::ai::shared_backend()?;
    let model = MODEL
        .get_or_init(|| {
            let model_params = LlamaModelParams::default();
            LlamaModel::load_from_file(backend, model_path, &model_params)
                .map_err(|error| format!("Failed to load chat model: {error}"))
        })
        .as_ref()
        .map_err(|error| anyhow!("{error}"))?;

    // Retrieval keeps prompts compact. Bound context/batches/threads so the
    // desktop app remains responsive and safely below its 2 GB RSS budget.
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(CHAT_CONTEXT_TOKENS))
        .with_n_batch(CHAT_BATCH_TOKENS)
        .with_n_ubatch(CHAT_UBATCH_TOKENS)
        .with_n_threads(CHAT_GENERATION_THREADS)
        .with_n_threads_batch(CHAT_PROMPT_THREADS);
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .map_err(|e| anyhow!("Failed to create context: {e}"))?;

    // Grammar-constrained sampling (AI spec §6.3): when a GBNF grammar is
    // supplied, generation is restricted to strings the grammar accepts. The
    // trailing greedy sampler then picks the highest-probability allowed token.
    let mut sampler: Option<llama_cpp_2::sampling::LlamaSampler> = None;
    if let Some(g) = grammar_gbnf {
        let grammar = llama_cpp_2::sampling::LlamaSampler::grammar(model, g, "root")
            .map_err(|e| anyhow!("Grammar init failed: {e}"))?;
        sampler = Some(llama_cpp_2::sampling::LlamaSampler::chain(
            [grammar, llama_cpp_2::sampling::LlamaSampler::greedy()],
            false,
        ));
    }

    let prompt = format!(
        "<|im_start|>system\n{system_prompt}<|im_end|>\n\
         <|im_start|>user\n{user_message}<|im_end|>\n\
         <|im_start|>assistant\n"
    );
    let prompt = match mode {
        CompletionMode::Default => prompt,
        CompletionMode::DirectDocumentAnswer => format!("{prompt}<think>\n\n</think>\n\n"),
    };

    let mut tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| anyhow!("Tokenization failed: {e}"))?;

    let n_ctx = ctx.n_ctx() as usize;
    // Keep generation room as well as prompt room. Never expose a context-size
    // error to callers: retain the prompt identity/header and its most recent
    // relevant content (including the user's question) when compression is
    // necessary. Document chat already ranks passages before this safety net.
    if tokens.len() >= n_ctx.saturating_sub(512) {
        let limit = n_ctx.saturating_sub(512);
        let prefix_len = limit.min(512).min(tokens.len());
        let suffix_len = limit
            .saturating_sub(prefix_len)
            .min(tokens.len().saturating_sub(prefix_len));
        let mut compressed = tokens[..prefix_len].to_vec();
        compressed.extend_from_slice(&tokens[tokens.len() - suffix_len..]);
        tokens = compressed;
    }

    // llama.cpp batches have a fixed capacity. Long document prompts must be
    // decoded in chunks rather than trying to put every prompt token in one
    // 512-token batch.
    const BATCH_SIZE: usize = CHAT_BATCH_TOKENS as usize;
    let mut batch = LlamaBatch::new(BATCH_SIZE, 1);
    let last_idx = (tokens.len() - 1) as i32;
    for (chunk_index, token_chunk) in tokens.chunks(BATCH_SIZE).enumerate() {
        batch.clear();
        let offset = chunk_index * BATCH_SIZE;
        for (index, token) in token_chunk.iter().enumerate() {
            let position = offset + index;
            batch
                .add(*token, position as i32, &[0], position as i32 == last_idx)
                .map_err(|e| anyhow!("Batch add: {e}"))?;
        }
        ctx.decode(&mut batch).map_err(|e| anyhow!("Decode: {e}"))?;
    }

    let mut output = String::new();
    let mut n_cur = tokens.len() as i32;
    let mut generated = 0i32;
    let mut first_token_elapsed_ms: Option<u128> = None;
    let n_predict = CHAT_MAX_GENERATED_TOKENS;
    let eos = model.token_eos();
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    loop {
        let new_token = match sampler.as_mut() {
            // Grammar path: the sampler builds candidates from the context
            // logits itself, applies the grammar, and records the accept step
            // that keeps its state machine in sync.
            Some(s) => s.sample(&ctx, batch.n_tokens() - 1),
            None => {
                let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
                let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
                candidates_p.sample_token_greedy()
            }
        };

        if new_token == eos || generated >= n_predict || n_cur >= n_ctx as i32 {
            break;
        }

        let piece = model
            .token_to_piece(new_token, &mut decoder, false, None)
            .map_err(|e| anyhow!("token_to_piece: {e}"))?;

        if first_token_elapsed_ms.is_none() {
            first_token_elapsed_ms = Some(started.elapsed().as_millis());
        }
        output.push_str(&piece);
        if !on_token(&piece) {
            break;
        }
        if identity_guard::buffer_has_leak(&output) {
            return Ok(identity_guard::canned().to_string());
        }

        batch.clear();
        batch
            .add(new_token, n_cur, &[0], true)
            .map_err(|e| anyhow!("Batch add: {e}"))?;
        ctx.decode(&mut batch).map_err(|e| anyhow!("Decode: {e}"))?;
        n_cur += 1;
        generated += 1;
    }

    eprintln!(
        "[ai] completion finished: prompt_tokens={} generated_tokens={} first_token_ms={} elapsed_ms={}",
        tokens.len(), generated, first_token_elapsed_ms.unwrap_or(0), started.elapsed().as_millis()
    );
    let output = match mode {
        CompletionMode::Default => output,
        CompletionMode::DirectDocumentAnswer => strip_thinking(&output),
    };
    Ok(identity_guard::guard(&output))
}

/// Defensive guard for a model that emits a thought closing marker despite the
/// pre-seeded empty block. Never expose hidden reasoning in document chat.
fn strip_thinking(raw: &str) -> String {
    raw.find("</think>")
        .map(|end| raw[end + "</think>".len()..].trim_start().to_string())
        .unwrap_or_else(|| raw.trim_start().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::prompts;
    use std::path::PathBuf;

    #[test]
    fn runtime_budget_reserves_a_core_and_bounds_generation() {
        assert!(CHAT_GENERATION_THREADS < CHAT_PROMPT_THREADS);
        assert!(CHAT_CONTEXT_TOKENS <= 3_072);
        assert!(CHAT_UBATCH_TOKENS < CHAT_BATCH_TOKENS);
        assert!(CHAT_MAX_GENERATED_TOKENS <= 128);
    }

    #[test]
    fn thinking_output_is_not_returned_for_document_answers() {
        assert_eq!(
            strip_thinking("<think>private work</think>Answer."),
            "Answer."
        );
        assert_eq!(strip_thinking("Direct answer."), "Direct answer.");
    }

    /// Opt-in real-world benchmark. It is ignored by default because it loads
    /// the local GGUF and performs genuine CPU inference. Set
    /// SOFFIT_AI_MODEL and one or more SOFFIT_AI_BENCHMARK_DOC_* variables.
    #[test]
    #[ignore = "requires a local GGUF model and user-supplied documents"]
    fn benchmark_user_documents() {
        let model = PathBuf::from(std::env::var("SOFFIT_AI_MODEL").expect("set SOFFIT_AI_MODEL"));
        let documents = [
            (
                "SOFFIT_AI_BENCHMARK_DOC_ONE",
                "Explain this document simply.",
            ),
            (
                "SOFFIT_AI_BENCHMARK_DOC_TWO",
                "What are the key service obligations and commercial terms?",
            ),
        ];

        for (variable, question) in documents {
            let Ok(path) = std::env::var(variable) else {
                continue;
            };
            let document = pdf_extract::extract_text(&path).expect("extract PDF text");
            // Match production's prompt budget. The current implementation
            // ranks a longer document before applying this bound.
            let excerpt: String = document.chars().take(3_600).collect();
            let name = PathBuf::from(&path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let system = prompts::document_chat(&name, &excerpt);
            let started = Instant::now();
            let answer =
                complete_document(&model, &system, question, |_| true).expect("complete document");
            eprintln!(
                "[ai-benchmark] file={name} elapsed_ms={} answer={answer}",
                started.elapsed().as_millis()
            );
            assert!(!answer.is_empty());
        }
    }
}
