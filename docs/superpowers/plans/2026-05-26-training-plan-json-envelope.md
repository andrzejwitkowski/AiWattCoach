# Training Plan JSON Envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change training-plan LLM generation and correction to use a JSON envelope with `plan` and optional `description`, parse only `plan`, persist `description` for debug visibility, and log bounded description metadata.

**Architecture:** Reuse the repo's existing schema-driven LLM output pattern from workout summary coaching. Keep the Intervals-style workout grammar as raw text inside `plan`, add a small training-plan LLM output contract in the domain layer, parse that envelope in the LLM adapter, persist optional descriptions in the training-plan operation record, and leave the downstream day parsing/correction flow focused on `plan` only.

**Tech Stack:** Rust, schemars, serde, existing training-plan domain service, existing LLM adapter prompt assembly, Mongo operation repository, Rust unit/integration tests.

---

## Scope Summary

This implementation should change only the training-plan LLM response contract and the related parsing/persistence/logging/test surfaces.

Expected files:
- Create: `src/domain/training_plan/llm_output.rs`
- Modify: `src/domain/training_plan/model.rs`
- Modify: `src/domain/training_plan/mod.rs`
- Modify: `src/adapters/llm/training_plan_generator.rs`
- Modify: `src/adapters/mongo/training_plan_generation_operations.rs`
- Modify: `src/domain/training_plan/service/mod.rs`
- Modify: `tests/llm_adapters/training_plan.rs`
- Modify: `tests/training_plan_mongo.rs`
- Modify: `tests/training_plan_service/validation.rs`
- Optionally modify: `reviewers.md`
- Optionally modify: `tasks/lessons.md`
- After implementation: `graphify-out/**` via `./scripts/rebuild_graphify.sh`

Non-goals:
- no rewrite of `parse_window(...)` into a structured JSON day parser
- no REST contract change for saved workout summary endpoints
- no change to the 14-day contiguous validation logic
- no new tool definitions

---

## Task 1: Add The Training Plan JSON Output Contract

**Files:**
- Create: `src/domain/training_plan/llm_output.rs`
- Modify: `src/domain/training_plan/mod.rs`

- [ ] **Step 1: Write the failing contract test first**

Add a focused unit test in the new file that proves the schema contract can parse a valid payload with `plan` and optional `description`, and rejects unknown fields.

Code to add in `src/domain/training_plan/llm_output.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::{parse_training_plan_llm_envelope, training_plan_llm_envelope_json_schema};

    #[test]
    fn training_plan_llm_envelope_parses_valid_payload() {
        let parsed = parse_training_plan_llm_envelope(
            r#"{"plan":"2026-04-06\nRest Day","description":"keep it light"}"#,
        )
        .expect("expected valid envelope");

        assert_eq!(parsed.plan, "2026-04-06\nRest Day");
        assert_eq!(parsed.description.as_deref(), Some("keep it light"));
    }

    #[test]
    fn training_plan_llm_envelope_rejects_unknown_fields() {
        let error = parse_training_plan_llm_envelope(
            r#"{"plan":"2026-04-06\nRest Day","extra":"nope"}"#,
        )
        .expect_err("expected unknown field rejection");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn training_plan_llm_envelope_schema_disallows_additional_properties() {
        let schema = training_plan_llm_envelope_json_schema();

        assert!(schema.contains(r#"\"plan\""#));
        assert!(schema.contains(r#"\"description\""#));
        assert!(schema.contains(r#"\"additionalProperties\": false"#));
    }
}
```

- [ ] **Step 2: Run the focused unit test and confirm failure**

Run:
```bash
cargo test training_plan_llm_envelope --lib -- --nocapture
```

Expected:
- fail because the module and helpers do not exist yet

- [ ] **Step 3: Implement the contract module**

Create `src/domain/training_plan/llm_output.rs` with:
```rust
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

use super::TrainingPlanError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrainingPlanLlmEnvelope {
    pub plan: String,
    pub description: Option<String>,
}

pub fn parse_training_plan_llm_envelope(
    payload: &str,
) -> Result<TrainingPlanLlmEnvelope, TrainingPlanError> {
    let parsed: TrainingPlanLlmEnvelope = serde_json::from_str(payload)
        .map_err(|error| TrainingPlanError::Unavailable(format!("invalid training plan llm json: {error}")))?;

    if parsed.plan.trim().is_empty() {
        return Err(TrainingPlanError::Unavailable(
            "training plan llm json missing non-empty plan".to_string(),
        ));
    }

    Ok(parsed)
}

pub fn training_plan_llm_envelope_json_schema() -> String {
    serde_json::to_string_pretty(&schema_for!(TrainingPlanLlmEnvelope))
        .expect("training plan llm envelope schema should serialize")
}
```

- [ ] **Step 4: Export the new contract from `src/domain/training_plan/mod.rs`**

Update exports so the adapter can import:
```rust
mod llm_output;

pub use llm_output::{
    parse_training_plan_llm_envelope,
    training_plan_llm_envelope_json_schema,
    TrainingPlanLlmEnvelope,
};
```

- [ ] **Step 5: Re-run the focused unit test and make sure it passes**

Run:
```bash
cargo test training_plan_llm_envelope --lib -- --nocapture
```

Expected:
- PASS for the new envelope tests

---

## Task 2: Extend The Training Plan Domain Model For Optional Descriptions

**Files:**
- Modify: `src/domain/training_plan/model.rs`
- Test: `tests/training_plan_mongo.rs`

- [ ] **Step 1: Add a failing Mongo round-trip test for description fields**

Extend `tests/training_plan_mongo.rs` by adding assertions to the existing tool-loop round-trip test or a sibling test that expects these fields to survive persistence:
```rust
assert_eq!(found.raw_plan_description.as_deref(), Some("initial note"));
assert_eq!(found.raw_correction_description.as_deref(), Some("correction note"));
```

Use an operation built with the new setters from the next step.

- [ ] **Step 2: Run the focused Mongo test and confirm failure**

Run:
```bash
cargo test --test training_plan_mongo training_plan_generation_operation_repository_round_trips_completed_tool_loop_state -- --nocapture
```

Expected:
- fail because the new fields/setters do not exist yet

- [ ] **Step 3: Add the new fields to `TrainingPlanGenerationOperation`**

In `src/domain/training_plan/model.rs`, add:
```rust
pub raw_plan_description: Option<String>,
pub raw_correction_description: Option<String>,
```

Initialize them in all constructors/cloners such as:
- `pending(...)`
- `reclaim(...)`
- `clone_pending_update(...)`

with:
```rust
raw_plan_description: None,
raw_correction_description: None,
```

or cloned values where appropriate.

- [ ] **Step 4: Add dedicated setters that preserve phase separation**

Add methods shaped like:
```rust
pub fn with_raw_plan_payload(
    &self,
    raw_plan_response: String,
    raw_plan_description: Option<String>,
    tool_loop_state: LlmToolLoopState,
    recorded_at_epoch_seconds: i64,
) -> Self

pub fn with_correction_payload(
    &self,
    raw_correction_response: String,
    raw_correction_description: Option<String>,
    tool_loop_state: LlmToolLoopState,
    recorded_at_epoch_seconds: i64,
) -> Self
```

Implementation rule:
- behave like the current `with_raw_plan_response(...)` / `with_correction_response(...)`
- also set the matching description field
- keep attempt-record behavior unchanged

After adding them, remove or replace the old raw-response-only setters if they are no longer used.

- [ ] **Step 5: Re-run the focused Mongo test and make sure it passes**

Run:
```bash
cargo test --test training_plan_mongo training_plan_generation_operation_repository_round_trips_completed_tool_loop_state -- --nocapture
```

Expected:
- PASS with both description fields round-tripping

---

## Task 3: Persist The New Description Fields In Mongo

**Files:**
- Modify: `src/adapters/mongo/training_plan_generation_operations.rs`
- Test: `tests/training_plan_mongo.rs`

- [ ] **Step 1: Add failing persistence coverage if Task 2 used only domain assertions**

If Task 2 did not already fail at the repository boundary, add a repository-level assertion that reads the stored operation back from Mongo and checks:
```rust
assert_eq!(found.raw_plan_description.as_deref(), Some("initial note"));
assert_eq!(found.raw_correction_description.as_deref(), Some("correction note"));
```

- [ ] **Step 2: Add document fields in the Mongo operation document**

In `TrainingPlanGenerationOperationDocument`, add:
```rust
raw_plan_description: Option<String>,
raw_correction_description: Option<String>,
```

- [ ] **Step 3: Map the new fields both directions**

Update `map_operation_to_document(...)`:
```rust
raw_plan_description: operation.raw_plan_description.clone(),
raw_correction_description: operation.raw_correction_description.clone(),
```

Update `map_document_to_operation(...)`:
```rust
raw_plan_description: document.raw_plan_description,
raw_correction_description: document.raw_correction_description,
```

- [ ] **Step 4: Re-run the focused Mongo test**

Run:
```bash
cargo test --test training_plan_mongo training_plan_generation_operation_repository_round_trips_completed_tool_loop_state -- --nocapture
```

Expected:
- PASS

---

## Task 4: Change The LLM Adapter To Use The JSON Envelope

**Files:**
- Modify: `src/adapters/llm/training_plan_generator.rs`
- Modify: `tests/llm_adapters/training_plan.rs`

- [ ] **Step 1: Add failing prompt-level assertions for schema-based output**

Update `tests/llm_adapters/training_plan.rs` so `training_plan_generator_explains_dated_output_grammar_in_plan_prompts()` now expects:
```rust
assert!(initial_prompt.contains("training_plan_response_schema="));
assert!(initial_prompt.contains("Return your final answer as JSON only"));
assert!(initial_prompt.contains("Put only parser-friendly dated workout text in `plan`"));
assert!(initial_prompt.contains("Put any explanation in `description`"));

assert!(correction_prompt.contains("training_plan_response_schema="));
assert!(correction_prompt.contains("Return your final answer as JSON only"));
assert!(correction_prompt.contains("Only output corrected dated sections for the invalid dates in `plan`"));
```

Also update the request/response tests so the fake assistant payload is JSON:
```rust
assert_eq!(response.raw_response, "2023-11-15\nRest Day");
assert_eq!(response.description.as_deref(), Some("keep it easy"));
```

- [ ] **Step 2: Run the focused adapter test file and confirm failure**

Run:
```bash
cargo test --test llm_adapters training_plan -- --nocapture
```

Expected:
- fail because prompts still demand raw text and `TrainingPlanPhaseOutput` has no description

- [ ] **Step 3: Extend `TrainingPlanPhaseOutput` to carry optional description**

In `src/domain/training_plan/model.rs`, change:
```rust
pub struct TrainingPlanPhaseOutput {
    pub raw_response: String,
    pub tool_loop_state: LlmToolLoopState,
}
```

to:
```rust
pub struct TrainingPlanPhaseOutput {
    pub raw_response: String,
    pub description: Option<String>,
    pub tool_loop_state: LlmToolLoopState,
}
```

- [ ] **Step 4: Update the system prompt contract in the adapter**

In `src/adapters/llm/training_plan_generator.rs`:
- import `parse_training_plan_llm_envelope` and `training_plan_llm_envelope_json_schema`
- replace the raw-text-only wording in `TRAINING_PLAN_OUTPUT_GRAMMAR` with JSON-envelope wording that still preserves the workout grammar inside `plan`

Target wording direction:
```rust
const TRAINING_PLAN_OUTPUT_GRAMMAR: &str = "Critical rules: Return your final answer as JSON only matching the training plan response schema. Put only parser-friendly dated workout text in `plan`. Put any rationale, notes, or extra commentary in `description`. Do not place commentary before, after, or inside the dated workout text in `plan`. ...";
```

- [ ] **Step 5: Embed the schema in both system prompts**

Update:
```rust
fn training_plan_initial_window_system_prompt(...)
fn training_plan_correction_system_prompt(...)
```

so both include:
```rust
format!(
    "... training_plan_response_schema={} ...",
    training_plan_llm_envelope_json_schema(),
)
```

- [ ] **Step 6: Replace raw assistant-text extraction with envelope parsing**

Add an adapter helper like:
```rust
fn require_training_plan_envelope(
    response: &crate::domain::llm::LlmChatResponse,
) -> Result<TrainingPlanLlmEnvelope, TrainingPlanError> {
    let payload = response
        .assistant_text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| TrainingPlanError::Unavailable("LLM returned no assistant text".to_string()))?;

    parse_training_plan_llm_envelope(payload)
}
```

Then in both generation methods return:
```rust
let envelope = require_training_plan_envelope(&response.response)?;

Ok(TrainingPlanPhaseOutput {
    raw_response: envelope.plan,
    description: envelope.description,
    tool_loop_state: response.state,
})
```

- [ ] **Step 7: Re-run the focused adapter tests and make them pass**

Run:
```bash
cargo test --test llm_adapters training_plan -- --nocapture
```

Expected:
- PASS for updated prompt/schema assertions
- PASS for JSON envelope parsing path

---

## Task 5: Preserve Descriptions In The Training Plan Service

**Files:**
- Modify: `src/domain/training_plan/service/mod.rs`
- Modify: `tests/training_plan_service/validation.rs`

- [ ] **Step 1: Add a failing service regression that proves `description` is ignored for parsing but persisted**

Extend the new regression in `tests/training_plan_service/validation.rs` or add a sibling test so the stub generator result includes:
```rust
TrainingPlanPhaseOutput {
    raw_response: plan_with_invalid_day(...),
    description: Some("short rationale".to_string()),
    tool_loop_state: LlmToolLoopState::default(),
}
```

Assertions should prove:
```rust
assert_eq!(operation.raw_plan_description.as_deref(), Some("short rationale"));
assert_eq!(operation.raw_correction_description.as_deref(), Some("fixed invalid day"));
assert_eq!(correction_inputs[0].0, "2026-04-10\nBroken session\n- nope".to_string());
```

- [ ] **Step 2: Run the focused service regression and confirm failure**

Run:
```bash
cargo test --test training_plan_service preamble_before_first_date_still_reaches_correction_flow -- --nocapture
```

Expected:
- fail because descriptions are not persisted yet

- [ ] **Step 3: Persist initial description when storing the plan payload**

In `src/domain/training_plan/service/mod.rs`, update the initial generation path from:
```rust
let raw_plan_tool_loop_state = raw_plan_response.tool_loop_state;
let raw_plan_response = raw_plan_response.raw_response;
operation = service.operations.upsert(
    operation.with_raw_plan_response(...)
).await?;
```

to the equivalent split form:
```rust
let raw_plan_tool_loop_state = raw_plan_response.tool_loop_state;
let raw_plan_description = raw_plan_response.description;
let raw_plan_response = raw_plan_response.raw_response;
operation = service
    .operations
    .upsert(operation.with_raw_plan_payload(
        raw_plan_response.clone(),
        raw_plan_description.clone(),
        raw_plan_tool_loop_state,
        service.clock.now_epoch_seconds(),
    ))
    .await?;
```

- [ ] **Step 4: Persist correction description the same way**

Make the matching change in the correction path using `with_correction_payload(...)`.

- [ ] **Step 5: Add bounded metadata logging in both phases**

Near the point where the envelope-derived payload is stored, add logs like:
```rust
tracing::info!(
    operation_key = %operation.operation_key,
    phase = "initial_generation",
    has_description = raw_plan_description.is_some(),
    description_chars = raw_plan_description.as_ref().map(|v| v.chars().count()).unwrap_or(0),
    plan_chars = raw_plan_response.chars().count(),
    description_preview = raw_plan_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(120).collect::<String>())
        .unwrap_or_default(),
    "stored training plan llm envelope"
);
```

Repeat for `phase = "correction"`.

Keep the preview helper local and bounded.

- [ ] **Step 6: Re-run the focused service regression**

Run:
```bash
cargo test --test training_plan_service validation -- --nocapture
```

Expected:
- PASS for the updated validation behavior tests

---

## Task 6: Update The Training Plan Test Doubles To Model Descriptions

**Files:**
- Modify: `tests/training_plan_service/support/mod.rs`
- Modify: `tests/llm_adapters/training_plan.rs`

- [ ] **Step 1: Adjust the stub training-plan generator to return `description: None` by default**

In `tests/training_plan_service/support/mod.rs`, update both generator methods from:
```rust
response.map(|raw_response| TrainingPlanPhaseOutput {
    raw_response,
    tool_loop_state: LlmToolLoopState::default(),
})
```

to:
```rust
response.map(|raw_response| TrainingPlanPhaseOutput {
    raw_response,
    description: None,
    tool_loop_state: LlmToolLoopState::default(),
})
```

- [ ] **Step 2: Adjust any recovery/custom generators constructing `TrainingPlanPhaseOutput`**

Search for every `TrainingPlanPhaseOutput {` literal and add:
```rust
description: None,
```

unless the test intentionally exercises description persistence.

- [ ] **Step 3: Re-run a narrow compile/test target to catch missed literals**

Run:
```bash
cargo check --tests
```

Expected:
- PASS after all struct literals are updated

---

## Task 7: Final Verification And Review Loop

**Files:**
- Review: all touched files
- Possibly modify: `reviewers.md`
- Possibly modify: `tasks/lessons.md`

- [ ] **Step 1: Run focused tests for the touched behavior**

Run:
```bash
cargo test --test llm_adapters training_plan -- --nocapture
```

Run:
```bash
cargo test --test training_plan_mongo training_plan_generation_operation_repository_round_trips_completed_tool_loop_state -- --nocapture
```

Run:
```bash
cargo test --test training_plan_service validation -- --nocapture
```

Expected:
- PASS for the touched LLM adapter, Mongo, and training-plan validation flows

- [ ] **Step 2: Run repo-required Rust verification gates**

Run:
```bash
bun run verify:arch
```

Run:
```bash
cargo fmt --all --check
```

Run:
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:
- all pass cleanly

- [ ] **Step 3: Rebuild graphify after code changes**

Run:
```bash
./scripts/rebuild_graphify.sh
```

Expected:
- `graphify-out/GRAPH_REPORT.md` and `graphify-out/graph.json` updated

- [ ] **Step 4: Perform the repo-required 4-iteration review loop**

For each of 4 iterations:
- review as strict reviewer
- review as very strict reviewer
- review as nitpicker
- fix only confirmed issues
- rerun the most relevant focused verification command

Do not skip this loop unless the user narrows verification explicitly.

- [ ] **Step 5: Update reusable review lessons if needed**

If implementation uncovered a new reusable mistake pattern, append it to:
- `reviewers.md`
- `tasks/lessons.md`

Only add a lesson if it is specific and reusable.

- [ ] **Step 6: Commit**

After verification passes, stage only the intended files and commit with a message like:
```bash
git add src/domain/training_plan/llm_output.rs src/domain/training_plan/model.rs src/domain/training_plan/mod.rs src/adapters/llm/training_plan_generator.rs src/adapters/mongo/training_plan_generation_operations.rs src/domain/training_plan/service/mod.rs tests/llm_adapters/training_plan.rs tests/training_plan_mongo.rs tests/training_plan_service/validation.rs graphify-out/GRAPH_REPORT.md graphify-out/graph.json

git commit -m "fix: structure training plan llm output"
```

If you intentionally do not want `graphify-out/**` in the commit, omit those paths explicitly and say so in the final summary.
