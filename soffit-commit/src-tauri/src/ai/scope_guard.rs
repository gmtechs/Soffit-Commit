/// Scope gate (§8.2) — checks user message against in-scope feature intents
/// using embedding similarity before letting it reach the generation model.
use crate::ai::embeddings::{cosine_similarity, SCOPE_THRESHOLD};

const OUT_OF_SCOPE_RESPONSE: &str =
    "I can help with your files, syncs, and data in Soffit Commit — that's outside what I can answer here.";

/// Example in-scope anchor phrases per feature (used for similarity comparison).
pub const IN_SCOPE_ANCHORS: &[&str] = &[
    // 6.1 conflict
    "explain this sync conflict",
    "why did this file conflict",
    "what caused the conflict",
    // 6.2 SQL error
    "explain this SQL error",
    "what does this error mean",
    "why is my query failing",
    // 6.3 NL→SQL
    "write a query to",
    "show me all rows where",
    "find records that",
    "create a SQL query for",
    // 6.4 activity summary
    "what changed this week",
    "summarize recent activity",
    "what happened to my files",
    // 6.5 semantic search
    "find files about",
    "search for",
    // 6.6 data insights
    "what do these results show",
    "summarize this data",
    "insights about this spreadsheet",
];

/// Check if the user message is in scope.
/// `query_embedding` is the embedding of the user message.
/// `anchor_embeddings` are the pre-embedded anchor phrases (cached at startup).
/// Returns true if in scope, false if the gate should block.
pub fn is_in_scope(query_embedding: &[f32], anchor_embeddings: &[Vec<f32>]) -> bool {
    if anchor_embeddings.is_empty() {
        // No anchors loaded — pass through (fail open during setup)
        return true;
    }
    anchor_embeddings.iter()
        .any(|anchor| cosine_similarity(query_embedding, anchor) >= SCOPE_THRESHOLD)
}

pub fn out_of_scope_response() -> &'static str { OUT_OF_SCOPE_RESPONSE }
