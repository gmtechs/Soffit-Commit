/// Layer 2 identity guard — scans generated text for banned model/lab names
/// and replaces the whole response with the Soffit AI canned line if found.

const BANNED_TERMS: &[&str] = &[
    "qwen",
    "alibaba",
    "tongyi",
    "qianwen",
    "openai",
    "anthropic",
    "google deepmind",
    "meta ai",
    "mistral ai",
    "llama",
    "hugging face",
    "huggingface",
];

const CANNED_RESPONSE: &str = "I'm Soffit AI, built by Laocta Techlabs.";

/// Returns the response unchanged if clean, or the canned response if any banned term found.
pub fn guard(text: &str) -> String {
    let lower = text.to_lowercase();
    for term in BANNED_TERMS {
        if lower.contains(term) {
            return CANNED_RESPONSE.to_string();
        }
    }
    text.to_string()
}

/// Check a streaming token buffer. Returns true if the buffer should be discarded.
pub fn buffer_has_leak(buffer: &str) -> bool {
    let lower = buffer.to_lowercase();
    BANNED_TERMS.iter().any(|t| lower.contains(t))
}

pub fn canned() -> &'static str {
    CANNED_RESPONSE
}
