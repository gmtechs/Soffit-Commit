/// Soffit AI — offline, in-process AI layer powered by llama-cpp-2.
/// All six features from the spec are routed through this module.
pub mod embeddings;
pub mod grammar;
pub mod identity_guard;
pub mod llm;
pub mod model_manager;
pub mod prompts;
pub mod scope_guard;
