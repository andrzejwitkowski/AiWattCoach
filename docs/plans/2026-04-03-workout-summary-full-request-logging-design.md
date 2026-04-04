# Workout Summary Full Request Logging Design

Goal: log the full `LlmChatRequest` built by the workout summary coach so the console shows exactly what the workout context builder produced before provider mapping.

Chosen approach:
- add one explicit structured `INFO` log in `src/adapters/llm/workout_summary_coach.rs`
- emit the full builder output fields unredacted for workout-summary requests only
- keep provider clients unchanged because the user wants the request as produced by the workout context builder, not the transformed provider JSON

Why this approach:
- shows the exact `system_prompt`, `stable_context`, `volatile_context`, and `conversation` leaving the workout-summary adapter
- keeps the change local to the workout-summary flow instead of affecting all LLM traffic
- avoids fighting the existing redacted `Debug` implementation for `LlmChatRequest`

Verification plan:
- add an adapter test that captures tracing logs and asserts the new log contains the full request content
- run `cargo test --test llm_adapters`, `cargo fmt --all --check`, and `cargo clippy --all-targets --all-features -- -D warnings`
