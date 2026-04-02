# Provider Logging Rollout Design

OpenRouter debugging proved that provider-local diagnostics are the fastest way to distinguish transport errors, upstream rejections, and response-shape mismatches. The remaining LLM adapters should expose the same level of diagnostics so future failures do not require iterative instrumentation.

Chosen approach:
- add the same balanced logging style to `src/adapters/llm/openai/client.rs`
- add the same balanced logging style to `src/adapters/llm/gemini/client.rs`
- keep changes local to adapter files and avoid behavior changes
- log request metadata, transport failures, non-success responses with truncated bodies, and parse/mapping failures where applicable

Why this approach:
- consistent debugging signal across all live providers
- preserves thin REST handlers and current error mapping semantics
- avoids broad refactors or shared logging abstractions until duplication becomes a real problem

Verification plan:
- run focused OpenAI, Gemini, and REST integration tests after the logging changes
- keep formatting clean with `cargo fmt --all --check`
