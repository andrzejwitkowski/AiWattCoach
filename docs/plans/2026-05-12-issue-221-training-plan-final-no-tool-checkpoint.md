# Training-plan final no-tool checkpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Issue:** https://github.com/andrzejwitkowski/AiWattCoach/issues/221

**Goal:** Close the training-plan crash window where a final no-tool LLM response can be lost after the provider reply but before later persistence, and prove recovery finishes without a second provider call.

**Architecture:** Extend the shared tool-loop durable state just enough to represent a completed final assistant response, checkpoint that state before returning from the no-tool terminal path, and let training-plan recovery resume directly from that completed state. Keep the issue scoped to training-plan behavior, with only small enabling refactors in shared tool-loop code.

**Tech Stack:** Rust, serde, Axum backend domain services, Mongo persistence, cargo test, clippy.

---

## File structure

**Modify**
- `src/domain/llm_tools/mod.rs` — add completed-tool-loop durable state and checkpoint/restore behavior for the final no-tool path.
- `src/domain/training_plan/model.rs` — store the richer durable tool-loop state without adding training-plan-specific duplication.
- `src/adapters/llm/training_plan_generator.rs` — consume restored completed tool-loop state and avoid another provider call.
- `src/domain/training_plan/service/mod.rs` — recover from completed tool-loop state even when raw plan/correction text was not yet persisted.
- `src/adapters/mongo/training_plan_generation_operations.rs` — round-trip the new `LlmToolLoopState` shape.

**Test**
- `tests/training_plan_service/recovery.rs` — crash-boundary and no-second-provider-call proof.
- `tests/llm_adapters/training_plan.rs` — shared tool-loop behavior for restored completed state.
- `tests/training_plan_mongo.rs` — repository round-trip for updated state.

---

### Task 1: Lock the failure mode with a training-plan recovery regression

**Files:**
- Modify: `tests/training_plan_service/recovery.rs`
- Test: `tests/training_plan_service/recovery.rs`

- [ ] **Step 1: Add a failing recovery test for the exact crash window**

Add a test that models:
- first attempt reaches a final assistant response with no tool calls
- the final completed tool-loop state is checkpointed
- the workflow fails before `raw_plan_response` persistence finishes
- second attempt reuses the stored state
- provider/generator call count does not increase on retry

Test shape:
- use a generator fake that records initial-plan call count
- first call returns a completed no-tool response state but then forces a failure after checkpoint persistence
- second call should not invoke the provider path again if recovery is correct

Suggested test name:
- `reclaim_reuses_completed_initial_tool_loop_state_without_second_provider_call`

- [ ] **Step 2: Run the targeted test to verify it fails for the right reason**

Run:
```bash
cargo test reclaim_reuses_completed_initial_tool_loop_state_without_second_provider_call --test training_plan_service -- --nocapture
```

Expected:
- FAIL because completed final no-tool tool-loop state is not yet durably recoverable without another provider call

- [ ] **Step 3: Commit the failing test**

```bash
git add tests/training_plan_service/recovery.rs
git commit -m "test: lock training-plan final no-tool crash boundary"
```

---

### Task 2: Extend durable tool-loop state for completed no-tool responses

**Files:**
- Modify: `src/domain/llm_tools/mod.rs`
- Test: `tests/llm_adapters/training_plan.rs`

- [ ] **Step 1: Add a minimal serializable completed-response payload to tool-loop state**

Change `LlmToolLoopState` so it can represent a completed terminal response, not just in-progress transcript state.

Constraints:
- keep it generic to the tool loop, not training-plan-specific
- include only what is needed to reconstruct the final `LlmChatResponse` without another provider call
- prefer a small nested struct over stuffing unrelated fields at the top level

Suggested shape:
- `completed_response: Option<CompletedLlmToolLoopResponse>` on `LlmToolLoopState`
- nested struct carries:
  - `provider`
  - `model`
  - `message`
  - `finish_reason`
  - `provider_request_id`
  - `usage`
  - `cache`

- [ ] **Step 2: Update `run_tool_loop_with_checkpoint(...)` terminal behavior**

In `src/domain/llm_tools/mod.rs`:
- when `response.tool_calls().is_empty()`, build final state including `completed_response`
- invoke `checkpoint(state.clone()).await?` before returning
- return `LlmToolLoopOutput` with that final state

Also add a restore fast-path:
- if `restored_state.completed_response.is_some()` and the state is already terminal, return reconstructed output immediately without calling `chat_port.chat(...)`

Do **not** change unrelated tool-call-round behavior.

- [ ] **Step 3: Add focused adapter/unit tests for the shared tool loop**

In `tests/llm_adapters/training_plan.rs`, add regressions proving:
- final no-tool responses invoke the checkpoint before returning
- checkpoint failure returns an error instead of handing back the assistant response
- restored completed state returns successfully
- restored completed state makes no second outbound provider request

Suggested test names:
- `training_plan_generator_checkpoints_final_no_tool_response_before_returning`
- `training_plan_generator_returns_error_when_final_checkpoint_fails`
- `training_plan_generator_reuses_completed_tool_loop_state_without_second_chat_call`

- [ ] **Step 4: Run targeted tests to verify the shared tool-loop change passes**

Run:
```bash
cargo test training_plan_generator_checkpoints_final_no_tool_response_before_returning --test llm_adapters -- --nocapture
cargo test training_plan_generator_returns_error_when_final_checkpoint_fails --test llm_adapters -- --nocapture
cargo test training_plan_generator_reuses_completed_tool_loop_state_without_second_chat_call --test llm_adapters -- --nocapture
cargo test reclaim_reuses_completed_initial_tool_loop_state_without_second_provider_call --test training_plan_service -- --nocapture
```

Expected:
- PASS

- [ ] **Step 5: Commit the shared tool-loop change**

```bash
git add src/domain/llm_tools/mod.rs tests/llm_adapters/training_plan.rs tests/training_plan_service/recovery.rs
git commit -m "fix: checkpoint completed no-tool tool-loop responses"
```

---

### Task 3: Teach training-plan generation and recovery to consume completed state

**Files:**
- Modify: `src/adapters/llm/training_plan_generator.rs`
- Modify: `src/domain/training_plan/service/mod.rs`
- Test: `tests/training_plan_service/recovery.rs`

- [ ] **Step 1: Update training-plan generator to accept restored completed tool-loop state**

In `src/adapters/llm/training_plan_generator.rs`:
- keep using `run_tool_loop_with_checkpoint(...)`
- rely on the new shared restore fast-path when `restored_state` is terminal
- continue producing `TrainingPlanPhaseOutput { raw_response, tool_loop_state }`
- derive `raw_response` from the restored completed response text when the loop returns from restored state

- [ ] **Step 2: Update training-plan service recovery logic**

In `src/domain/training_plan/service/mod.rs`:
- when a stale/reclaimed operation has `initial_plan_tool_loop_state` or `correction_tool_loop_state` containing a completed response but the raw text fields are still absent, resume from that state instead of triggering another provider call
- keep later persistence ordering intact: local durable state first, projection work after
- do not broaden scope into scheduler cleanup

- [ ] **Step 3: Add a second focused correction-path regression if needed**

Only if the initial fix leaves correction behavior asymmetric, add:
- `reclaim_reuses_completed_correction_tool_loop_state_without_second_provider_call`

If the initial-path regression plus shared tool-loop test already proves the shared behavior adequately, skip this extra test to avoid duplication.

- [ ] **Step 4: Run targeted recovery tests**

Run:
```bash
cargo test --test training_plan_service recovery -- --nocapture
```

Expected:
- existing recovery tests still PASS
- new no-second-provider-call regression PASS

- [ ] **Step 5: Commit the training-plan recovery update**

```bash
git add src/adapters/llm/training_plan_generator.rs src/domain/training_plan/service/mod.rs tests/training_plan_service/recovery.rs
git commit -m "fix: resume training-plan generation from completed tool-loop state"
```

---

### Task 4: Round-trip the richer state through Mongo

**Files:**
- Modify: `src/adapters/mongo/training_plan_generation_operations.rs`
- Test: `tests/training_plan_mongo.rs`

- [ ] **Step 1: Update the Mongo document mapping for the richer tool-loop state**

Make sure the new `LlmToolLoopState` field shape serializes/deserializes cleanly in:
- `initial_plan_tool_loop_state`
- `correction_tool_loop_state`

Keep the mapping explicit and backward-compatible where practical.

- [ ] **Step 2: Add a repository round-trip regression**

Add a Mongo test that persists an operation with a completed tool-loop state and reads it back unchanged.

Suggested test name:
- `training_plan_generation_operation_repository_round_trips_completed_tool_loop_state`

Assert:
- completed response metadata survives round-trip
- transcript and round count survive round-trip

- [ ] **Step 3: Run the targeted Mongo test**

Run:
```bash
cargo test training_plan_generation_operation_repository_round_trips_completed_tool_loop_state --test training_plan_mongo -- --nocapture
```

Expected:
- PASS

- [ ] **Step 4: Commit the persistence update**

```bash
git add src/adapters/mongo/training_plan_generation_operations.rs tests/training_plan_mongo.rs
git commit -m "test: round-trip completed tool-loop state in training-plan ops"
```

---

### Task 5: Final verification

**Files:**
- Verify only

- [ ] **Step 1: Run the most relevant targeted tests together**

Run:
```bash
cargo test reclaim_reuses_completed_initial_tool_loop_state_without_second_provider_call --test training_plan_service -- --nocapture
cargo test training_plan_generator_reuses_completed_tool_loop_state_without_second_chat_call --test llm_adapters -- --nocapture
cargo test training_plan_generation_operation_repository_round_trips_completed_tool_loop_state --test training_plan_mongo -- --nocapture
```

Expected:
- all PASS

- [ ] **Step 2: Run required repo verification**

Run:
```bash
bun run verify:arch
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:
- all PASS

- [ ] **Step 3: Sanity-check issue scope before calling it done**

Confirm the branch did **not** also absorb:
- A2 shared `LlmError` serialization
- A3 scheduler parse helper extraction
- A4 public tool-call materialization dedup
- A5 provider-transcript merge helper dedup
- A6 runtime worker wiring cleanup

- [ ] **Step 4: Commit final polish if needed**

```bash
git add -A
git commit -m "chore: finish issue 221 verification"
```

---

## Self-review

### Spec coverage
- Covers durable final no-tool checkpointing before return.
- Covers restored completed-state fast-path without second provider call.
- Covers training-plan recovery path specifically.
- Covers Mongo round-trip for any state-shape change.

### Placeholder scan
- No `TODO` / `TBD` placeholders.
- Commands and file paths are concrete.

### Type consistency
- Uses the existing `LlmToolLoopState` / `TrainingPlanPhaseOutput` naming.
- Keeps the change centered on shared tool-loop state plus training-plan recovery.
