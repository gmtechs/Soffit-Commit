# Soffit Commit — local AI integration spec

Status: ready for implementation
Companion to: `soffit-commit-ux-bugfix-spec.md`
Scope: model selection, download/packaging, in-process Rust architecture, grounding design for all 6 AI features, and the "always identify as Laocta Techlabs" requirement.

---

## 1. What's being built

An in-app, offline, CPU-only AI layer that powers six specific grounded features (not an open chatbot). Everything below is scoped to make hallucination structurally unlikely, not just prompted-away — see §7.

---

## 2. Model selection

### 2.1 Chat/reasoning model — Qwen3-0.6B-Instruct (GGUF, Apache 2.0)
- Source: `bartowski/Qwen_Qwen3-0.6B-GGUF` on Hugging Face — imatrix-quantized builds of the official `Qwen/Qwen3-0.6B` release.
- Quant to ship: **Q4_K_M** (not explicitly listed in the repo's size table, but sits between the published `IQ3_XS` 0.38GB and `Q5_K_L` 0.65GB entries — expect roughly **0.45–0.5GB**). If the agent wants a documented exact size, fall back to `Q5_K_L` (0.65GB, listed as "high quality, recommended" by the quantizer) for a small size/quality tradeoff.
- Download URL pattern (direct HTTPS, no auth needed — public Apache-2.0 repo):
  `https://huggingface.co/bartowski/Qwen_Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q4_K_M.gguf`
  (swap the filename for `Qwen3-0.6B-Q5_K_L.gguf` etc. if a different quant is picked)
- License: Apache 2.0 — safe for commercial redistribution alongside the app.

### 2.2 Embedding model — Qwen3-Embedding-0.6B (GGUF, Apache 2.0)
- Source: `Qwen/Qwen3-Embedding-0.6B-GGUF` (official Qwen org repo).
- Quant to ship: **Q8_0** (~0.6–0.7GB, the quant used in the reference notebook for this exact model).
- Download URL pattern:
  `https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf`
- This model does **not** generate text — it turns text into a vector for similarity comparison. It cannot hallucinate in the generative sense, which is why it's the backbone of §6.5 (search) and §8.2 (scope gating).

### 2.3 Total optional download size
~1.0–1.2GB combined. Not bundled in the base installer — see §5.

---

## 3. Inference engine — llama-cpp-2 (in-process Rust crate)

**Decision: use the `llama-cpp-2` crate, in-process, no sidecar process.**

Why this over alternatives:
- It's a maintained, lightly-wrapped Rust binding to llama.cpp (`docs.rs/llama-cpp-2`), with first-class GGUF loading and a **grammar module** — this is what makes constrained/JSON-safe output (§6.3) possible without extra tooling.
- There's direct precedent for exactly this stack: a Tauri v2 desktop app (Rust shell + WebView frontend) shipping `llama.cpp` via the `llama-cpp-2` crate for in-process, offline LLM inference with no server and no network dependency after model download — the same shape as Soffit Commit.
- Running in-process (vs. spawning a `llama-server` sidecar binary) avoids managing a second process, a port, and cross-platform binary bundling for the server executable. Everything lives in one Rust binary.
- Alternative considered: `mistral.rs` (pure Rust, sometimes faster on GPU) — rejected for v1 because this deployment is CPU-only and `llama-cpp-2`'s grammar/GBNF support is more battle-tested for the structured-output requirement in §6.3.

### 3.1 Cargo setup
```toml
[dependencies]
llama-cpp-2 = "0.1"
```
(Pin to a specific patch version once the agent starts building, and vendor/build llama.cpp per that crate's build instructions — it compiles the C++ backend as part of the Rust build.)

### 3.2 Module layout (new, under `src-tauri/src/ai/`)
```
ai/
  mod.rs              orchestration entrypoint, feature router
  model_manager.rs     download, checksum verification, load/unload, app_data_dir paths
  llm.rs               llama-cpp-2 wrapper: context, chat template, streaming
  embeddings.rs         embedding generation + cosine similarity search
  grammar.rs           GBNF grammars for JSON/SQL-safe structured output
  identity_guard.rs     post-generation identity/branding safety net (§7)
  scope_guard.rs        in-scope/out-of-scope gate using embeddings (§8.2)
  prompts/              one prompt template file per feature (§6)
```

### 3.3 Streaming to the frontend
Use Tauri's event system (`app_handle.emit("ai-token", token)`) to stream generated tokens to the React side as they're produced, so the chat panel can render live typing instead of waiting for the full completion.

---

## 4. Chat template

Qwen3 models use a ChatML-style template. The system message is where the identity and grounding rules live (§7, §8). General shape:
```
<|im_start|>system
{system_prompt}
<|im_end|>
<|im_start|>user
{user_message}
<|im_end|>
<|im_start|>assistant
```
Every feature in §6 builds its own `{system_prompt}` — they are not interchangeable; each is scoped tightly to its task and its injected context.

---

## 5. Download & packaging strategy

- **Do not bundle the GGUF files in the installer.** Ship the app without them; the AI layer is opt-in.
- On first attempt to use any AI feature (or from a dedicated Settings → AI panel), check `app_data_dir()/models/` for both files.
- If missing, show a download prompt stating the size (~1.1GB total) and let the user confirm before pulling from the Hugging Face URLs in §2.
- Verify each downloaded file against a SHA256 checksum pinned in the app's code (fetch the checksum from the Hugging Face repo at build time, hardcode it — don't trust an unverified download silently).
- Store models in the OS-appropriate Tauri app data directory, not inside the app bundle, so they survive app updates and can be deleted independently.
- Settings → AI panel shows: model status (not downloaded / downloading with progress / ready), disk space used, and a "remove models" action.

---

## 6. Feature-by-feature grounding design

Every feature follows the same shape: **pull real data → inject as context → constrain the question the model is allowed to answer → (optionally) constrain the output format.** None of them let the model answer from open memory.

### 6.1 Explain a sync conflict
- Context pulled: the conflict's file path, both version metadata (timestamps, device, size), and if available a text diff snippet.
- System prompt: "You are explaining a file sync conflict using only the data below. Do not guess at causes not shown in the data. If the data doesn't explain why the conflict happened, say so plainly."
- Output: free text, 2–4 sentences, no grammar constraint needed.

### 6.2 Explain a SQL error
- Context pulled: the failing SQL text (or the relevant few lines around the failure offset) + the raw engine error string.
- System prompt: "Explain this SQL error in plain language using only the query and error text given. Point to the specific line/statement likely responsible. Do not invent SQL features that weren't in the query."
- This directly targets the phpMyAdmin-dump bug case — the model should be able to say "this fails because `SET` here is a MySQL session command your engine doesn't recognize" once you feed it the dump content and error, without needing to "know" MySQL trivia from training — it's reasoning over the text you gave it.

### 6.3 Natural language → draft SQL
- Context pulled: the current table schema (column names/types) for the active file/connection.
- Output: **constrained via GBNF grammar** to strict JSON: `{"sql": "...", "explanation": "..."}`. Reject/retry generation if it doesn't validate against the grammar.
- After generation, run the produced SQL through a syntax-only parser check (not execution) before ever showing it as a suggestion.
- Hard rule: **never auto-executed.** Always surfaces in the existing diff/preview flow from the SQL console spec.

### 6.4 Activity summary ("what changed this week")
- Context pulled: raw rows from the Activity log for the requested time window.
- System prompt: "Summarize only the activity log entries below. Do not reference files, peers, or events not listed."
- Output: free text, short paragraph or bullet list.

### 6.5 Semantic file/peer search (embedding model, not chat model)
- No generation involved. File names/paths/peer labels are embedded once (cached, re-embedded on change) using Qwen3-Embedding-0.6B; the search query is embedded at query time; results ranked by cosine similarity.
- This is the most hallucination-proof feature in the set — it's pure vector math, not text generation.

### 6.6 Data insights over SQL/Excel result sets
- Context pulled: pre-computed stats from your own code (row count, min/max/avg of numeric columns, null counts) — **compute these in Rust, not the model.**
- System prompt: "Phrase the following pre-computed statistics in plain language. Do not calculate anything yourself — only describe the numbers given."
- This keeps arithmetic reliability outside the model entirely, where a 0.6B model would be weakest.

---

## 7. Branding requirement — always "made by Laocta Techlabs"

Research note: relying on the system prompt alone to override a model's trained self-identity is known to be **brittle** — models can and do leak their real base identity (e.g., "I am Qwen, developed by Alibaba Cloud") under direct or adversarial questioning, even with instructions in place. So this needs three layers, not one:

### Layer 1 — System prompt (baseline, required)
Every system prompt built in §6 is prefixed with an identity block:
```
You are Soffit AI, an assistant built into Soffit Commit by Laocta Techlabs.
If asked who made you, who trained you, or what model you are, answer only:
"I'm Soffit AI, built by Laocta Techlabs."
Never mention Qwen, Alibaba, Alibaba Cloud, Tongyi Qianwen, or any other AI lab or model name.
```

### Layer 2 — Output guard (required, non-negotiable)
Implement `identity_guard.rs`: after generation completes (or per-chunk during streaming, buffered), scan the text for a fixed banned-term list: `qwen`, `alibaba`, `tongyi`, plus generic lab names (`openai`, `anthropic`, `google`, `meta ai`, `mistral ai`) — case-insensitive. If any match is found, **discard the entire response** and replace it with the canned line: *"I'm Soffit AI, built by Laocta Techlabs."* Don't try to edit the leaked phrase in place — a full discard-and-replace is simpler and can't leave a half-corrected sentence.
Also scrub any raw model filename/metadata (e.g. `Qwen3-0.6B-Q4_K_M.gguf`) from **any** UI-facing surface, including debug panels — this is a separate leak path from the generated text itself.

### Layer 3 — Optional identity fine-tune (recommended for v2, not required for launch)
A small LoRA fine-tune on a few hundred synthetic Q&A pairs ("who are you" / "what model are you" / "who trained you" → the Soffit AI / Laocta Techlabs answer) is cheap to produce on a 0.6B model and would make Layer 1 far more reliable on its own, reducing how often Layer 2 needs to fire. Not required for MVP since Layers 1+2 together already prevent any leak from reaching the user — but worth scheduling once the core feature set is stable.

---

## 8. Hard-coded non-hallucination rules (not just prompted)

### 8.1 Context-only instruction, every feature
Every system prompt in §6 includes some form of: *"Answer only using the information given above. If it's not enough to answer, say so — do not use outside knowledge."* This is necessary but, per Layer-2 logic above, not sufficient on its own — pair prompted behavior with structural constraints wherever possible (grammar constraints in §6.3, pre-computed stats in §6.6, no-generation search in §6.5).

### 8.2 Scope gate before generation
Before any user-typed chat message reaches the generation model, check it against the six in-scope feature intents using the **embedding model** (compare the message's embedding against a small fixed set of example in-scope questions per feature). If similarity is below a set threshold for all of them, skip the generation model entirely and respond with a canned out-of-scope message: *"I can help with your files, syncs, and data in Soffit Commit — that's outside what I can answer here."* This is a second gate, independent of the model's own judgment, and it also saves a generation call for messages that were never going to get a grounded answer anyway.

### 8.3 SQL is never auto-executed
Restated from §6.3: any AI-drafted SQL always lands in the existing diff/preview UI. No feature in this spec is allowed to write to disk, run a query, or modify a file without the existing human-review flow already speced for the SQL console and Excel grid.

---

## 9. Acceptance checklist

- [ ] Qwen3-0.6B-Instruct GGUF (Q4_K_M or Q5_K_L) and Qwen3-Embedding-0.6B-GGUF (Q8_0) download and load successfully from the exact URLs in §2
- [ ] Models are not bundled in the installer; first-use opt-in download implemented with progress + checksum verification
- [ ] `llama-cpp-2` integrated in-process under `src-tauri/src/ai/`, no sidecar process
- [ ] Streaming tokens reach the frontend via Tauri events
- [ ] All 6 features (§6.1–§6.6) implemented with their specific context-injection + system prompt, not a shared generic prompt
- [ ] NL→SQL drafts are grammar-constrained to valid JSON and never auto-executed
- [ ] Data-insight stats are computed in Rust, not by the model
- [ ] Semantic search uses the embedding model only, no generation call
- [ ] Identity Layer 1 (system prompt) present in every feature's prompt
- [ ] Identity Layer 2 (output guard) implemented and tested against direct "who made you" / "are you Qwen" style prompts
- [ ] No raw model filename/metadata leaks into any UI-facing surface, including debug views
- [ ] Scope gate (§8.2) intercepts out-of-scope chat messages before they reach the generation model
- [ ] Settings → AI panel shows model status, disk usage, and a remove-models action