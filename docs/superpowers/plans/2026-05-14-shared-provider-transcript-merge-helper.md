# Shared Provider Transcript Merge Helper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract one shared `merge_provider_transcript_with_retry(...)` helper so workout-summary and coach-conversation provider-transcript persistence use the same optimistic retry and compare-and-set merge behavior.

**Architecture:** Keep the shared retry/merge logic in `src/domain/llm/persistence.rs` and keep workflow-specific repository reads, timestamp computation, and repository writes in the existing services. Prefer a closure-based helper over a new shared repository trait so the extraction stays inside the existing domain boundaries and does not widen the port surface.

**Tech Stack:** Rust 2021, Tokio, tracing, existing hexagonal domain ports, cargo fmt, clippy, focused Rust integration tests.

---

## File Map

- Create/expand: `src/domain/llm/persistence.rs`
- Modify: `src/domain/workout_summary/service/internals/recovery.rs`
- Modify: `src/domain/coach_conversation/service/internals/persistence.rs`
- Modify: `src/domain/workout_summary/service/use_cases/chat.rs`
- Modify: `src/domain/coach_conversation/service/use_cases.rs`
- Verify existing support stays unchanged: `src/domain/workout_summary/service/internals/messaging.rs`
- Verify existing support stays unchanged: `src/domain/coach_conversation/service/internals/persistence.rs`
- Test: `tests/workout_summary_service/messaging.rs`
- Test: `tests/calendar_coach_service/transcript_persistence.rs`

### Design Constraints

- Keep `src/domain/llm/persistence.rs` as the only shared home for transcript-merge retry behavior.
- Do not add a new shared repository trait unless the closure-based helper proves impossible.
- Preserve current retry semantics exactly:
  - `POST_PROVIDER_WRITE_ATTEMPTS == 2`
  - `backoff_base_ms == 25`
  - retry only on retryable repository/CAS errors
  - re-read latest persisted transcript on each retry before merging
- Keep workflow-specific timestamp generation in the existing workflow helpers so `next_provider_transcript_updated_at_epoch_seconds(...)` behavior does not drift.
- Do not change persist-before-side-effects ordering in either reply workflow.

### Task 1: Add Shared Transcript Merge Retry Helper

**Files:**
- Modify: `src/domain/llm/persistence.rs`

- [ ] **Step 1: Add focused shared types for latest transcript snapshot and merge-retry inputs**

Add a small helper snapshot type near the existing retry types so callers can pass their latest persisted transcript state without creating a new repository abstraction.

```rust
pub struct ProviderTranscriptSnapshot {
    pub provider_transcript: Vec<LlmChatMessage>,
    pub expected_updated_at_epoch_seconds: i64,
}
```

- [ ] **Step 2: Add the shared `merge_provider_transcript_with_retry(...)` helper**

Implement a new helper in `src/domain/llm/persistence.rs` that wraps `retry_persist(...)`, reloads the latest transcript on each attempt, merges with `merge_provider_transcript_entries(...)`, and then delegates persistence to a caller-supplied closure.

Target shape:

```rust
pub async fn merge_provider_transcript_with_retry<TLoad, TPersist, LoadFut, PersistFut, E>(
    config: RetryConfig,
    incoming_provider_transcript: Vec<LlmChatMessage>,
    is_retryable: impl Fn(&E) -> bool,
    mut load_latest: TLoad,
    mut persist_merged: TPersist,
    ctx: &RetryContext,
) -> Result<(), E>
where
    TLoad: FnMut() -> LoadFut,
    TPersist: FnMut(ProviderTranscriptSnapshot, Vec<LlmChatMessage>) -> PersistFut,
    LoadFut: Future<Output = Result<ProviderTranscriptSnapshot, E>> + Send,
    PersistFut: Future<Output = Result<(), E>> + Send,
    E: std::fmt::Display,
{
    retry_persist(config, is_retryable, || {
        let incoming_provider_transcript = incoming_provider_transcript.clone();
        Box::pin(async {
            let latest = load_latest().await?;
            let merged = merge_provider_transcript_entries(
                latest.provider_transcript.clone(),
                &incoming_provider_transcript,
            );
            persist_merged(latest, merged).await
        })
    }, ctx)
    .await
}
```

The exact generic names can vary, but the semantics above must hold.

- [ ] **Step 3: Add helper unit tests directly in `src/domain/llm/persistence.rs`**

Add tests that verify the helper itself, not just the downstream workflows.

Coverage targets:

```rust
#[tokio::test]
async fn merge_provider_transcript_with_retry_reloads_latest_state_after_retryable_conflict() {
    // first persist attempt returns retryable repository error
    // second load sees transcript containing "Concurrent update"
    // final persisted transcript contains both previous and incoming messages
}

#[tokio::test]
async fn merge_provider_transcript_with_retry_does_not_retry_non_retryable_error() {
    // first persist attempt returns non-retryable error
    // helper returns immediately
    // load/persist counters show only one attempt
}
```

- [ ] **Step 4: Run focused helper tests**

Run:

```bash
cargo test merge_provider_transcript_with_retry -- --nocapture
```

Expected: both new helper tests pass.

### Task 2: Switch Workout Summary To The Shared Helper

**Files:**
- Modify: `src/domain/workout_summary/service/internals/recovery.rs`
- Verify no behavior change needed in: `src/domain/workout_summary/service/internals/messaging.rs`
- Test: `tests/workout_summary_service/messaging.rs`

- [ ] **Step 1: Replace the local transcript merge loop in workout summary**

Update `src/domain/workout_summary/service/internals/recovery.rs` so the method named `merge_provider_transcript_with_retry(...)` delegates to the new shared helper instead of duplicating the retry loop inline.

The workflow should still:

```rust
let summary = svc.get_existing_summary(&uid, &wid).await?;
let latest = ProviderTranscriptSnapshot {
    provider_transcript: summary.provider_transcript,
    expected_updated_at_epoch_seconds: summary.updated_at_epoch_seconds,
};
svc.replace_provider_transcript(
    &uid,
    &wid,
    latest.expected_updated_at_epoch_seconds,
    merged,
).await
```

- [ ] **Step 2: Keep retry configuration and error classification unchanged**

Preserve the existing values:

```rust
RetryConfig {
    max_attempts: POST_PROVIDER_WRITE_ATTEMPTS,
    backoff_base_ms: 25,
}
```

and:

```rust
|e| matches!(e, WorkoutSummaryError::Repository(_))
```

- [ ] **Step 3: Re-run the existing workout summary transcript persistence regression**

Run:

```bash
cargo test --test workout_summary_service generate_coach_reply_retries_provider_transcript_write_after_compare_and_set_conflict -- --nocapture
```

Expected: pass, with the final stored transcript still containing `Turn 0`, `Concurrent summary update`, and the new reply turn.

### Task 3: Switch Coach Conversation To The Shared Helper

**Files:**
- Modify: `src/domain/coach_conversation/service/internals/persistence.rs`
- Modify if import cleanup is needed: `src/domain/coach_conversation/service/use_cases.rs`
- Test: `tests/calendar_coach_service/transcript_persistence.rs`

- [ ] **Step 1: Replace the local transcript merge loop in coach conversation**

Update `src/domain/coach_conversation/service/internals/persistence.rs` so its local `merge_provider_transcript_with_retry(...)` method delegates to the shared helper.

The caller-specific closures should still do:

```rust
let latest = svc
    .conversations
    .find_by_user_id_and_conversation_id(&uid, &cid)
    .await?
    .ok_or(CoachConversationError::NotFound)?;

let latest = ProviderTranscriptSnapshot {
    provider_transcript: latest.provider_transcript.clone(),
    expected_updated_at_epoch_seconds: latest.updated_at_epoch_seconds,
};
```

and persist with the existing `replace_provider_transcript(&latest_conversation, merged)` path or an equivalent path that preserves current timestamp logic.

- [ ] **Step 2: Preserve current tracing and failure behavior at the use-case layer**

Do not change the higher-level failure mapping in:

```rust
src/domain/coach_conversation/service/use_cases.rs
```

The use case must still convert transcript persistence failure into:

```rust
LlmError::Internal(format!(
    "failed to persist provider transcript after provider response: {error}"
))
```

- [ ] **Step 3: Re-run the existing calendar coach transcript persistence regression**

Run:

```bash
cargo test --test calendar_coach_service calendar_coach_retries_provider_transcript_write_after_compare_and_set_conflict -- --nocapture
```

Expected: pass, with the final stored transcript still containing `Turn 0`, `Concurrent calendar update`, and `Coach reply`.

### Task 4: Final Verification And Graph Refresh

**Files:**
- Modify only if verification or imports force small cleanup in the files above
- Refresh generated graph output after code changes

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: no formatting diffs.

- [ ] **Step 2: Run clippy for the touched Rust codebase**

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: `Finished` or success output with zero warnings promoted to errors.

- [ ] **Step 3: Run architecture verification**

Run:

```bash
bun run verify:arch
```

Expected: architecture verification succeeds.

- [ ] **Step 4: Refresh graphify output required by repo instructions**

Run:

```bash
./scripts/rebuild_graphify.sh
```

Expected: `graphify-out/` refresh completes without errors.

- [ ] **Step 5: Review the diff narrowly before asking for review or committing**

Run:

```bash
git diff -- src/domain/llm/persistence.rs \
  src/domain/workout_summary/service/internals/recovery.rs \
  src/domain/coach_conversation/service/internals/persistence.rs \
  src/domain/workout_summary/service/use_cases/chat.rs \
  src/domain/coach_conversation/service/use_cases.rs \
  tests/workout_summary_service/messaging.rs \
  tests/calendar_coach_service/transcript_persistence.rs
```

Expected: only the planned helper extraction and targeted test updates appear.

## Notes For The Implementer

- The smallest correct extraction is closure-based. Resist introducing a new `ProviderTranscriptMergeRepository` trait unless the compiler proves the closure version unworkable.
- Keep the workflow-local methods named `merge_provider_transcript_with_retry(...)` if that keeps call sites stable; they can become thin wrappers over the shared helper.
- Do not move timestamp computation into the generic helper. That detail belongs to the workflow-specific repository write helpers that already own `updated_at` semantics.
- Do not widen the helper to handle unrelated operation persistence. This task is only about shared provider-transcript merge retry behavior.
