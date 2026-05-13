# A2 — Shared typed LlmError checkpoint serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Issue:** https://github.com/andrzejwitkowski/AiWattCoach/issues/224

**Goal:** Replace three independently duplicated `LlmError` ↔ `{ kind, message }` serialization ladders in the scheduler families with one shared mapping, eliminating drift risk without changing any public error behavior.

**Architecture:** Promote `LlmReplyOperationFailureKind` into a shared `SerializedLlmError` that lives under `src/domain/llm/` alongside two free conversion functions. Each scheduler keeps its own feature-specific outer error enum; only the `Llm` variant mapping chain switches to the shared helpers.

**Tech Stack:** Rust, serde, thiserror-free (existing codebase style).

---

## Current code findings

### Duplication is exact across all three schedulers

The `serialize_xxx_error(...)` match ladder:

```
LlmError::CredentialsNotConfigured → "credentials_not_configured"
LlmError::ProviderNotConfigured    → "provider_not_configured"
LlmError::ModelNotConfigured       → "model_not_configured"
LlmError::ContextTooLarge(_)       → "context_too_large"
LlmError::UnsupportedProvider(_)   → "unsupported_provider"
LlmError::Transport(_)             → "transport"
LlmError::ProviderRejected(_)      → "provider_rejected"
LlmError::RateLimited(_)           → "rate_limited"
LlmError::InvalidResponse(_)       → "invalid_response"
LlmError::Checkpoint(_)            → "checkpoint"
LlmError::Internal(_)              → "internal"
```

appears identically in:
- `src/domain/workout_summary/service/scheduler/mod.rs`
- `src/domain/athlete_summary/service/scheduler.rs`
- `src/domain/coach_conversation/service/scheduler.rs`

The deserialization ladder is also identical except for minor fallback string differences (e.g. `"transport error"` vs `"transport"`).

### Shared representation already exists

`LlmReplyOperationFailureKind` (`src/domain/llm/operation.rs`) already has:
- an enum with a variant per `LlmError`
- `from_llm_error(error: &LlmError) -> Self`
- `to_llm_error(&self, message: Option<String>) -> LlmError`

It is currently used only by `LlmReplyOperation` for durable reply-operations state. It is serde-ready. The scheduler families can reuse it directly — no new type needed.

---

## Plan

### Task 1: Expose shared LlmError serialization helpers from domain/llm

**Files:**
- Modify: `src/domain/llm/operation.rs`
- Modify: `src/domain/llm/mod.rs`

- [ ] **Step 1: Rename `LlmReplyOperationFailureKind` to `SerializedLlmError`**

Change:
```rust
pub enum LlmReplyOperationFailureKind { ... }
```
to:
```rust
pub enum SerializedLlmError { ... }
```

Update all existing call sites inside `operation.rs` and any outside callers (grep for the old name).

- [ ] **Step 2: Add two free public functions**

```rust
pub fn serialize_llm_error(error: &LlmError) -> SerializedLlmError {
    SerializedLlmError::from_llm_error(error)
}

pub fn deserialize_llm_error(kind: SerializedLlmError) -> LlmError {
    kind.to_llm_error(None) // callers supply message where needed
}
```

- [ ] **Step 3: Build a focused unit test proving round-trip for every LlmError variant**

In `src/domain/llm/operation.rs` (test module), add:

```rust
#[test]
fn serialized_llm_error_round_trips_every_variant() {
    // one case per variant with message and without
    let cases: Vec<LlmError> = vec![
        LlmError::CredentialsNotConfigured,
        LlmError::ProviderNotConfigured,
        LlmError::ModelNotConfigured,
        LlmError::ContextTooLarge("ctx".into()),
        LlmError::UnsupportedProvider("up".into()),
        LlmError::Transport("t".into()),
        LlmError::ProviderRejected("pr".into()),
        LlmError::RateLimited("rl".into()),
        LlmError::InvalidResponse("ir".into()),
        LlmError::Checkpoint("ck".into()),
        LlmError::Internal("i".into()),
    ];
    for original in cases {
        let serialized = serialize_llm_error(&original);
        // assert kind matches, assert message round-trips through to_llm_error
    }
}
```

- [ ] **Step 4: Run tests + verify existing reply-operation code still compiles**

Run:
```bash
cargo test serialized_llm_error_round_trips_every_variant -- --nocapture
```

Expected: PASS. No existing reply-operation tests break.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor: extract shared SerializedLlmError from LlmReplyOperationFailureKind"
```

---

### Task 2: Replace workout-summary LlmError serialization

**Files:**
- Modify: `src/domain/workout_summary/service/scheduler/mod.rs`

- [ ] **Step 1: Replace the inlined match ladder with shared helpers**

In `serialize_workout_summary_error(...)`:
```rust
WorkoutSummaryError::Llm(error) => SerializedWorkoutSummaryError::Llm {
    error_kind: serialize_llm_error(error).kind_str().to_string(),
    message: /* same as before */,
},
```

In `deserialize_workout_summary_error(...)`:
```rust
SerializedWorkoutSummaryError::Llm { error_kind, message } => {
    WorkoutSummaryError::Llm(deserialize_llm_error(
        SerializedLlmError::from_kind_str(&error_kind)
            .map(|kind| kind.with_message(message))
            .unwrap_or_else(|| LlmError::Internal(message.unwrap_or_else(|| "internal llm error".to_string())))
    ))
}
```

- [ ] **Step 2: Verify existing workout-summary scheduler tests still pass**

Run:
```bash
cargo test --test training_plan_service scheduler_backed -- --nocapture
```

Or more targeted:
```bash
cargo test scheduler_backed_generate_coach_reply -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor: use shared SerializedLlmError in workout-summary scheduler"
```

---

### Task 3: Replace athlete-summary LlmError serialization

**Files:**
- Modify: `src/domain/athlete_summary/service/scheduler.rs`

Same mechanical replacement as workout summary.

- [ ] **Step 1: Replace the inlined match ladder**

- [ ] **Step 2: Verify tests:**
```bash
cargo test scheduler_backed_generate_summary -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor: use shared SerializedLlmError in athlete-summary scheduler"
```

---

### Task 4: Replace coach-conversation LlmError serialization

**Files:**
- Modify: `src/domain/coach_conversation/service/scheduler.rs`

Same mechanical replacement.

- [ ] **Step 1: Replace the inlined match ladder**

- [ ] **Step 2: Verify tests:**
```bash
cargo test parse_failed_restores_serialized_llm_error_from_task_checkpoint -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor: use shared SerializedLlmError in coach-conversation scheduler"
```

---

### Task 5: Remove leftover dead code and final verification

**Files:**
- Verify: remove any now-unused local helpers in the three scheduler files

- [ ] **Step 1: Clean up dead imports and unused helper functions**

After replacing the ladders, the three scheduler modules should no longer import `LlmError` variants directly for serialization. Remove any dead `use ...::LlmError` imports that were only needed for the old match ladders.

- [ ] **Step 2: Run full scheduler test suites for all three families**

Run:
```bash
cargo test --test training_plan_service scheduler_backed -- --nocapture
cargo test scheduler_backed_generate_summary -- --nocapture
cargo test parse_failed_restores_serialized_llm_error_from_task_checkpoint -- --nocapture
```

- [ ] **Step 3: Run required repo verification**

```bash
bun run verify:arch
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git commit -m "chore: remove dead LlmError import paths after shared serialization refactor"
```

---

## Non-goals

- Do not touch `TrainingPlanError` — training plan does not use scheduled LLM error checkpoints.
- Do not fold outer feature-specific error enums into one type.
- Do not change the public error mapping in REST/websocket layers.
