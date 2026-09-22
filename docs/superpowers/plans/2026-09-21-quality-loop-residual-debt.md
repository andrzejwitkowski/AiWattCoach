# Quality-loop residual debt cleanup

> **For agentic workers:** Use superpowers:subagent-driven-development or executing-plans.

**Goal:** Clear the three thermonuclear residuals: attempt-arg sprawl, Adjustment-rules hitchhiking on the last plan day, and finalize raw-field alignment without `updated_at`.

**Architecture:** Nest quality-attempt state into typed ctx groups (same style as `QualityIdentity` / `QualityPlanning`); move the checklist gate onto envelope `description`; route shipped-best raw alignment through a proper operation transition that bumps timestamps.

**Tech Stack:** Rust training-plan domain + existing quality_loop / quality_prompt tests.

**Out of scope:** Removing the thin `self.render_plan_window` wrapper (explicitly leave it). Changing `plan_quality_pass_score` / raise_to_next *enforcement* beyond the checklist home. New Mongo fields for checklist text.

## Locked decisions

1. **Attempt args:** Introduce nested ctx structs in `service/quality/` so `run_quality_evaluation_attempts` / `run_one_quality_evaluation_attempt` / `finalize_plan_quality_loop` drop `#[allow(clippy::too_many_arguments)]`.
2. **Adjustment rules:** Live in `QualityDraft.description` (coach commentary), never in `plan` text. Checklist gate scans description only. Generator feedback copy and description guidance say so.
3. **Finalize timestamps:** Add `TrainingPlanGenerationOperation::with_shipped_best_raw(plan, description, recorded_at)` using `clone_pending_update` so `updated_at` / pending status bookkeeping stay consistent; finalize uses it instead of bare field writes.

---

## File map

| Debt | Files |
|------|--------|
| AttemptCtx | [`service/quality/mod.rs`](src/domain/training_plan/service/quality/mod.rs), [`service/quality/attempt.rs`](src/domain/training_plan/service/quality/attempt.rs) |
| Checklist → description | [`quality_prompt/feedback.rs`](src/domain/training_plan/quality_prompt/feedback.rs), [`prompt_guidance.rs`](src/domain/training_plan/prompt_guidance.rs), [`service/quality/mod.rs`](src/domain/training_plan/service/quality/mod.rs) (checklist call site), tests |
| Finalize transition | [`model.rs`](src/domain/training_plan/model.rs), finalize in [`service/quality/mod.rs`](src/domain/training_plan/service/quality/mod.rs) |

---

### Task 1: Nest quality attempt / finalize inputs

**Files:**
- Modify: `src/domain/training_plan/service/quality/mod.rs`
- Modify: `src/domain/training_plan/service/quality/attempt.rs`

- [x] **Step 1: Add ctx types**

```rust
pub(super) struct QualityLoopLimits {
    pub start_attempt: u32,
    pub max_loops: u32,
    pub pass_score: u8,
}

pub(super) struct QualityProgressSink<'a> {
    pub messages: &'a mut Vec<String>,
    pub port: Option<&'a Arc<dyn PlanQualityProgressPort>>,
}

pub(super) struct QualityAttemptLoopCtx<'a> {
    pub identity: &'a GenerationIdentity<'a>,
    pub planning: GenerationPlanning<'a>,
    pub snapshot: &'a mut TrainingPlanSnapshot,
    pub draft: &'a mut QualityDraft,
    pub operation: &'a mut TrainingPlanGenerationOperation,
    pub best: &'a mut Option<BestDraft>,
    pub progress: QualityProgressSink<'a>,
    pub limits: QualityLoopLimits,
}

struct QualityFinalizeInput<'a> {
    identity: &'a GenerationIdentity<'a>,
    snapshot: TrainingPlanSnapshot,
    operation: TrainingPlanGenerationOperation,
    best: Option<BestDraft>,
    accepted: bool,
    max_loops: u32,
    progress: QualityProgressSink<'a>,
}
```

Keep field counts ≤ 7 at each struct top level by nesting (`progress`, `limits`).

- [x] **Step 2: Rewire call sites**

`run_plan_quality_loop` builds `QualityAttemptLoopCtx` / `QualityFinalizeInput` and calls:

- `run_quality_evaluation_attempts(&mut ctx) -> Result<bool, …>`
- `finalize_plan_quality_loop(finalize) -> Result<PlanQualityLoopResult, …>`
- `run_one_quality_evaluation_attempt(ctx, attempt, availability_summary)`

Remove all three `#[allow(clippy::too_many_arguments)]` on these functions. Confirm with `cargo clippy -D warnings`.

- [x] **Step 3: Compile + quality_loop tests**

```bash
CARGO_BUILD_JOBS=1 cargo test -j 1 --test training_plan_service quality_loop -- --test-threads=1
```

---

### Task 2: Checklist lives in description

**Files:**
- Modify: `src/domain/training_plan/quality_prompt/feedback.rs`
- Modify: `src/domain/training_plan/prompt_guidance.rs` (`TRAINING_PLAN_DESCRIPTION_GUIDANCE`)
- Modify: `src/domain/training_plan/service/quality/mod.rs` (`regenerate_structurally_valid_snapshot`)
- Modify: `src/domain/training_plan/quality_prompt/mod.rs` tests
- Modify: `tests/training_plan_service/quality_loop.rs`

- [x] **Step 1: Change gate API**

```rust
pub fn draft_addresses_quality_checklist(
    draft_description: Option<&str>,
    raise_to_next: &str,
) -> bool {
    if raise_to_next.trim().is_empty() {
        return true;
    }
    draft_description
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("adjustment rules")
}
```

Update `format_quality_feedback` copy to say the checklist section must appear in the JSON `description` field (not `plan`).

- [x] **Step 2: Generator guidance**

Append to `TRAINING_PLAN_DESCRIPTION_GUIDANCE` (not OUTPUT_GRAMMAR): when addressing quality feedback / raise_to_next, put an `Adjustment rules` section in `description` only; do not append it to `plan`.

- [x] **Step 3: Call site**

```rust
if draft_addresses_quality_checklist(replan.draft.description.as_deref(), raise_to_next) {
```

Warn / sharper-feedback strings stay coherent (“omitted Adjustment rules in description”).

- [x] **Step 4: Tests**

- Unit: checklist fails when only plan text has the heading; passes when description has it.
- Integration: `quality_loop_retries_once_when_replan_omits_adjustment_rules` — first replan description `None`/without heading; second replan sets description containing `Adjustment rules` via `set_initial_plan_descriptions` (plan bodies stay pure windows).
- Keep a regression that appending “Adjustment rules” only onto rendered plan text does **not** satisfy the gate.

---

### Task 3: Finalize via operation transition

**Files:**
- Modify: `src/domain/training_plan/model.rs`
- Modify: `src/domain/training_plan/service/quality/mod.rs`
- Modify: model unit tests

- [x] **Step 1: Add transition**

```rust
pub fn with_shipped_best_raw(
    &self,
    raw_plan_response: String,
    raw_plan_description: Option<String>,
    recorded_at_epoch_seconds: i64,
) -> Self {
    let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
    updated.raw_plan_response = Some(raw_plan_response);
    updated.raw_plan_description = raw_plan_description;
    updated
}
```

Does not clear correction transcripts (same as align helper).

- [x] **Step 2: Finalize uses it**

```rust
if let Some(best_plan) = operation.best_quality_plan_response.clone() {
    let best_description = operation.best_quality_plan_description.clone();
    operation = operation.with_shipped_best_raw(
        best_plan,
        best_description,
        self.clock.now_epoch_seconds(),
    );
}
```

- [x] **Step 3: Unit test**

Assert `updated_at_epoch_seconds` / `last_attempt`-style fields from `clone_pending_update` move forward, and raw plan+description match best; correction fields untouched if present.

Optionally extend exhaustion quality_loop assertion that stored op `updated_at` advanced through finalize (already true via other upserts; focus model unit test).

---

### Task 4: Verification

```bash
export CARGO_BUILD_JOBS=1
cargo test -j 1 --lib quality_prompt -- --test-threads=1
cargo test -j 1 --lib operation_transition -- --test-threads=1
cargo test -j 1 --lib prompt_guidance -- --test-threads=1
cargo test -j 1 --test training_plan_service quality_loop -- --test-threads=1
bun run verify:arch
cargo fmt --all --check
cargo clippy -j 1 --all-targets --all-features -- -D warnings
```

Confirm zero new `clippy::too_many_arguments` allows in `service/quality/`.

---

## Acceptance

1. No `#[allow(clippy::too_many_arguments)]` on quality attempt/finalize helpers.
2. Checklist gate is description-only; plan-only “Adjustment rules” fails; description heading passes; quality_loop retry test uses descriptions queue.
3. Shipping best goes through `with_shipped_best_raw` and bumps operation timestamps via `clone_pending_update`.

## Self-check vs debt list

| Debt | Task |
|------|------|
| Attempt too-many-arguments | Task 1 |
| Adjustment rules on last day | Task 2 |
| Finalize without updated_at | Task 3 |
| Thin render wrapper | Explicitly out of scope |
