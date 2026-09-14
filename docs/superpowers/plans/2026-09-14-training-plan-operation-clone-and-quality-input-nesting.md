# Training-plan operation clone walls + quality loop input nesting

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete field-by-field `TrainingPlanGenerationOperation` copy walls via clone-then-mutate + one private transition helper, and nest `PlanQualityLoopInput` into identity / planning / draft+ports groups so quality orchestration stays scannable.

**Architecture:** Match the existing `let mut updated = self.clone();` idiom from `src/domain/planned_workout_wahoo_syncs/model.rs`. Keep public method signatures unchanged. Nest quality inputs only inside `service/quality.rs` (and the single call site in `service/mod.rs`); do not couple quality to `CorrectionRoundInput`.

**Tech Stack:** Rust 2021, existing `training_plan` domain + `training_plan_service` tests

**Out of scope:** Meso-cycle operations, evaluator race/event context, frontend AI-agents field tables, resume/seed simplification

**Commits:** Do not commit unless the user explicitly asks.

---

## File map

| Area | Create | Modify |
| --- | --- | --- |
| Operation model | — | `src/domain/training_plan/model.rs` |
| Quality loop inputs | — | `src/domain/training_plan/service/quality.rs` |
| Quality call site | — | `src/domain/training_plan/service/mod.rs` (~lines 901–914) |
| Optional model unit tests | — | `src/domain/training_plan/model.rs` (`#[cfg(test)]`) or keep coverage via `tests/training_plan_service` |

---

### Task 1: Lock operation transition behavior with a focused unit test

**Files:**
- Modify: `src/domain/training_plan/model.rs` (add `#[cfg(test)] mod tests` at end of file, before or after existing types if cleaner at bottom of `impl` file)

- [ ] **Step 1: Add failing/locking tests for reclaim / mark_completed / mark_failed**

Add at the bottom of `src/domain/training_plan/model.rs`:

```rust
#[cfg(test)]
mod operation_transition_tests {
    use super::{PlanQualityEvaluation, TrainingPlanGenerationOperation};
    use crate::domain::ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus};

    fn sample_operation() -> TrainingPlanGenerationOperation {
        let mut op = TrainingPlanGenerationOperation::pending(
            "training-plan:u:w:1".to_string(),
            "u".to_string(),
            "w".to_string(),
            100,
            200,
        );
        op.workout_recap_text = Some("recap".to_string());
        op.raw_plan_response = Some("plan".to_string());
        op.quality_evaluations = vec![PlanQualityEvaluation {
            attempt: 1,
            score: 6,
            critique: "tempo heavy".to_string(),
        }];
        op.best_quality_evaluation = Some(PlanQualityEvaluation {
            attempt: 1,
            score: 6,
            critique: "tempo heavy".to_string(),
        });
        op.best_quality_plan_response = Some("best-plan".to_string());
        op.attempt_count = 3;
        op
    }

    #[test]
    fn reclaim_keeps_payload_clears_failure_bumps_attempt() {
        let mut op = sample_operation();
        op.status = WorkflowStatus::Failed;
        op.failure = Some(super::TrainingPlanFailureState {
            phase: WorkflowPhase::Correction,
            message: "boom".to_string(),
        });

        let reclaimed = op.reclaim(500);
        assert_eq!(reclaimed.status, WorkflowStatus::Pending);
        assert!(reclaimed.failure.is_none());
        assert_eq!(reclaimed.attempt_count, 4);
        assert_eq!(reclaimed.last_attempt_at_epoch_seconds, 500);
        assert_eq!(reclaimed.updated_at_epoch_seconds, 500);
        assert_eq!(reclaimed.workout_recap_text.as_deref(), Some("recap"));
        assert_eq!(reclaimed.raw_plan_response.as_deref(), Some("plan"));
        assert_eq!(reclaimed.best_quality_plan_response.as_deref(), Some("best-plan"));
        assert_eq!(reclaimed.quality_evaluations.len(), 1);
        assert_eq!(reclaimed.created_at_epoch_seconds, 200);
    }

    #[test]
    fn mark_completed_preserves_payload_and_clears_failure() {
        let mut op = sample_operation();
        op.failure = Some(super::TrainingPlanFailureState {
            phase: WorkflowPhase::QualityEvaluation,
            message: "x".to_string(),
        });
        let completed = op.mark_completed(600);
        assert_eq!(completed.status, WorkflowStatus::Completed);
        assert!(completed.failure.is_none());
        assert_eq!(completed.updated_at_epoch_seconds, 600);
        assert_eq!(completed.attempt_count, 3);
        assert_eq!(completed.best_quality_plan_response.as_deref(), Some("best-plan"));
    }

    #[test]
    fn mark_failed_sets_failure_and_validation_issues() {
        let op = sample_operation();
        let failed = op.mark_failed(
            WorkflowPhase::Correction,
            "nope".to_string(),
            vec![ValidationIssue {
                scope: "2026-04-10".to_string(),
                message: "bad".to_string(),
            }],
            700,
        );
        assert_eq!(failed.status, WorkflowStatus::Failed);
        assert_eq!(
            failed.failure.as_ref().map(|f| f.message.as_str()),
            Some("nope")
        );
        assert_eq!(failed.validation_issues.len(), 1);
        assert_eq!(failed.updated_at_epoch_seconds, 700);
        assert_eq!(failed.raw_plan_response.as_deref(), Some("plan"));
    }
}
```

- [ ] **Step 2: Run the new tests (should pass on current code — behavior lock before refactor)**

```bash
export CARGO_BUILD_JOBS=1
cargo test --lib operation_transition_tests -- --test-threads=1
```

Expected: 3 passed

---

### Task 2: Replace clone walls with `transition` + clone-then-mutate

**Files:**
- Modify: `src/domain/training_plan/model.rs` (`impl TrainingPlanGenerationOperation`)

- [ ] **Step 1: Add private `transition` helper and rewrite the four methods**

Replace `reclaim`, `clone_pending_update`, `mark_completed`, and `mark_failed` bodies with:

```rust
fn transition(
    &self,
    status: WorkflowStatus,
    failure: Option<TrainingPlanFailureState>,
    updated_at_epoch_seconds: i64,
) -> Self {
    let mut updated = self.clone();
    updated.status = status;
    updated.failure = failure;
    updated.updated_at_epoch_seconds = updated_at_epoch_seconds;
    updated
}

pub fn reclaim(&self, now_epoch_seconds: i64) -> Self {
    let mut updated = self.transition(WorkflowStatus::Pending, None, now_epoch_seconds);
    updated.last_attempt_at_epoch_seconds = now_epoch_seconds;
    updated.attempt_count = self.attempt_count.saturating_add(1);
    updated
}

fn clone_pending_update(&self, updated_at_epoch_seconds: i64) -> Self {
    self.transition(WorkflowStatus::Pending, None, updated_at_epoch_seconds)
}

pub fn mark_completed(&self, updated_at_epoch_seconds: i64) -> Self {
    self.transition(WorkflowStatus::Completed, None, updated_at_epoch_seconds)
}

pub fn mark_failed(
    &self,
    phase: WorkflowPhase,
    message: String,
    validation_issues: Vec<ValidationIssue>,
    updated_at_epoch_seconds: i64,
) -> Self {
    let mut updated = self.transition(
        WorkflowStatus::Failed,
        Some(TrainingPlanFailureState { phase, message }),
        updated_at_epoch_seconds,
    );
    updated.validation_issues = validation_issues;
    updated
}
```

Delete the four giant `Self { ... field: self.field.clone() ... }` blocks entirely.

Leave `pending(...)` as an explicit constructor (it is not a copy of an existing op).

Leave all `with_*` methods unchanged — they already call `clone_pending_update` and only mutate the fields they own.

- [ ] **Step 2: Re-run transition unit tests**

```bash
export CARGO_BUILD_JOBS=1
cargo test --lib operation_transition_tests -- --test-threads=1
```

Expected: 3 passed

- [ ] **Step 3: Run recovery-focused service tests (reclaim / pending paths)**

```bash
export CARGO_BUILD_JOBS=1
cargo test --test training_plan_service recovery -- --test-threads=1
```

Expected: all matched tests passed

- [ ] **Step 4: Clippy on the touched crate targets**

```bash
export CARGO_BUILD_JOBS=1
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: no warnings/errors

---

### Task 3: Nest `PlanQualityLoopInput` into identity / planning groups

**Files:**
- Modify: `src/domain/training_plan/service/quality.rs`
- Modify: `src/domain/training_plan/service/mod.rs` (quality call site only)

- [ ] **Step 1: Replace flat `PlanQualityLoopInput` with nested groups**

In `src/domain/training_plan/service/quality.rs`, replace the flat struct with:

```rust
pub(super) struct QualityIdentity<'a> {
    pub user_id: &'a str,
    pub workout_id: &'a str,
    pub saved_at_epoch_seconds: i64,
    pub recap: &'a WorkoutRecap,
}

pub(super) struct QualityPlanning<'a> {
    pub planning_context: &'a mut Option<TrainingPlanPlanningContext>,
    pub planning_context_loaded: &'a mut bool,
}

pub(super) struct PlanQualityLoopInput<'a> {
    pub identity: QualityIdentity<'a>,
    pub planning: QualityPlanning<'a>,
    pub snapshot: TrainingPlanSnapshot,
    pub draft_plan_text: String,
    pub operation: TrainingPlanGenerationOperation,
    pub plan_quality_config: &'a Arc<dyn PlanQualityEvaluatorLlmConfigPort>,
    pub plan_quality_progress: Option<&'a Arc<dyn PlanQualityProgressPort>>,
}
```

- [ ] **Step 2: Update `run_plan_quality_loop` destructuring and usages**

At the top of `run_plan_quality_loop`:

```rust
let PlanQualityLoopInput {
    identity,
    planning,
    mut snapshot,
    mut draft_plan_text,
    mut operation,
    plan_quality_config,
    plan_quality_progress,
} = input;
let QualityIdentity {
    user_id,
    workout_id,
    saved_at_epoch_seconds,
    recap,
} = identity;
let QualityPlanning {
    planning_context,
    planning_context_loaded,
} = planning;
```

Keep the rest of the loop body using the same local names (`user_id`, `planning_context`, …) so the behavioral code stays untouched.

- [ ] **Step 3: Thread nested groups into `regenerate_structurally_valid_snapshot`**

Change the regenerate helper signature from 8 loose args to:

```rust
async fn regenerate_structurally_valid_snapshot(
    &self,
    identity: &QualityIdentity<'_>,
    planning: QualityPlanning<'_>,
    operation: &mut TrainingPlanGenerationOperation,
    quality_feedback: &str,
) -> Result<(TrainingPlanSnapshot, String), TrainingPlanError>
```

Inside, bind:

```rust
let QualityIdentity {
    user_id,
    workout_id,
    saved_at_epoch_seconds,
    recap,
} = identity;
let QualityPlanning {
    planning_context,
    planning_context_loaded,
} = planning;
```

Call site inside the quality loop becomes:

```rust
self.regenerate_structurally_valid_snapshot(
    &QualityIdentity {
        user_id,
        workout_id,
        saved_at_epoch_seconds,
        recap,
    },
    QualityPlanning {
        planning_context,
        planning_context_loaded,
    },
    &mut operation,
    &feedback,
)
.await
```

Do **not** change `CorrectionRoundInput` in this task. Keep building it from the same locals after regenerate’s planning muts are in scope.

Because `QualityPlanning` holds exclusive mutable refs, rebuild it only at the call site (do not store both `planning` and the reborrowed fields alive at once). Pattern:

```rust
// After outer destructure, use planning_context / planning_context_loaded locals.
// When calling regenerate, pass QualityPlanning { planning_context, planning_context_loaded }.
```

If the borrow checker complains about splitting `identity` while still needing `QualityIdentity` later, keep `identity` as a struct and pass `&identity` into regenerate, with planning locals separate (recommended above).

- [ ] **Step 4: Update the call site in `service/mod.rs`**

Replace the flat construction with:

```rust
.run_plan_quality_loop(quality::PlanQualityLoopInput {
    identity: quality::QualityIdentity {
        user_id: &user_id,
        workout_id: &workout_id,
        saved_at_epoch_seconds,
        recap: &recap,
    },
    planning: quality::QualityPlanning {
        planning_context: &mut planning_context,
        planning_context_loaded: &mut planning_context_loaded,
    },
    snapshot,
    draft_plan_text: raw_plan_response,
    operation,
    plan_quality_config: &plan_quality_config,
    plan_quality_progress: service.plan_quality_progress.as_ref(),
})
```

- [ ] **Step 5: Run quality + recovery suites and clippy**

```bash
export CARGO_BUILD_JOBS=1
cargo test --lib operation_transition_tests -- --test-threads=1
cargo test --test training_plan_service quality_loop -- --test-threads=1
cargo test --test training_plan_service -- --test-threads=1
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all green; clippy clean.

Note: run cargo commands **sequentially** on this host (`CARGO_BUILD_JOBS=1`, never parallel cargo).

---

### Task 4: Done checklist

- [ ] **Step 1: Confirm no field-by-field `Self { ... }` rebuilds remain** for `reclaim` / `clone_pending_update` / `mark_completed` / `mark_failed` in `model.rs` (search for `best_quality_plan_response: self.best_quality_plan_response.clone()` inside those four methods — should be zero hits).

- [ ] **Step 2: Confirm `PlanQualityLoopInput` top-level field count ≤ 7** (identity, planning, snapshot, draft, operation, config, progress).

- [ ] **Step 3: Rebuild graphify after code edits**

```bash
./scripts/rebuild_graphify.sh
```

---

## Self-review

1. **Spec coverage:** Finding 2 → Tasks 1–2. Finding 3 → Task 3. Done gate → Task 4.
2. **Placeholders:** None; concrete code and commands included.
3. **Type consistency:** `QualityIdentity` / `QualityPlanning` / `PlanQualityLoopInput` / `transition` names match across tasks.
4. **YAGNI:** No meso-cycle mirror, no `CorrectionRoundInput` merge, no public API changes.
