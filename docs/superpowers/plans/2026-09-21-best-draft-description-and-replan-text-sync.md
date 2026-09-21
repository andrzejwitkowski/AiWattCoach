# Best-draft description + replan plan-text sync

> **For agentic workers:** Use superpowers:subagent-driven-development or executing-plans. Steps use checkbox syntax.

**Goal:** Fix thermonuclear findings (1) best-draft commentary skew and (3) evaluator draft text diverging from structurally corrected days.

**Architecture:** Persist best plan text and best description together; re-render evaluator draft text from the post-correction day map so it matches the snapshot that will ship. Keep quality-loop `QualityDraft` as the single in-memory pair.

**Tech Stack:** Rust training-plan domain + Mongo operation document mapping.

**Out of scope:** pass_score / raise_to_next enforcement; changing OUTPUT_GRAMMAR; using `raw_correction_description` as full-plan commentary.

## Locked decisions

1. **Best draft:** add `best_quality_plan_description: Option<String>` on the operation (domain + Mongo). Always write it in the same `with_quality_evaluation` branch that writes `best_quality_plan_response`.
2. **Replan / initial draft text:** after `apply_correction_rounds` (and after initial structural corrections before the quality loop), set draft plan text via a new `render_plan_window(&BTreeMap<String, TrainingPlanDay>) -> String` that round-trips through existing `serialize_planned_workout` + rest-day formatting.
3. **Description after corrections:** keep envelope `raw_plan_description` (full-plan claims). Do **not** switch to `raw_correction_description`.
4. **Finalize coherence:** when shipping `best`, if best plan/description differ from current `raw_plan_*`, upsert operation so `raw_plan_response` / `raw_plan_description` match the shipped best (or at minimum ensure best fields are complete; prefer rewriting raw fields for operator readability).

---

## File map

| Area | Files |
|------|--------|
| Render helper | [`service/parsing.rs`](src/domain/training_plan/service/parsing.rs) (or small `service/render.rs`) |
| Replan return | [`service/quality/mod.rs`](src/domain/training_plan/service/quality/mod.rs) |
| Initial draft seed | [`service/mod.rs`](src/domain/training_plan/service/mod.rs) |
| Best persistence | [`model.rs`](src/domain/training_plan/model.rs), [`mongo/training_plan_generation_operations.rs`](src/adapters/mongo/training_plan_generation_operations.rs) |
| Attempt write path | [`service/quality/attempt.rs`](src/domain/training_plan/service/quality/attempt.rs) |
| Tests | `quality_prompt` N/A; unit tests for render; `tests/training_plan_service/quality_loop.rs`; mongo fixture fields; model transition tests |

---

### Task 1: `render_plan_window`

**Files:**
- Create or modify: `src/domain/training_plan/service/parsing.rs` (prefer colocating with `parse_window`)
- Test: same file `#[cfg(test)]` or focused unit module

- [ ] **Step 1: Implement renderer**

For each date-sorted day in the map:

- Rest: `YYYY-MM-DD\nRest Day` or `YYYY-MM-DD\nRest Day: {reason}`
- Workout: `YYYY-MM-DD\n{title}\n{serialize_planned_workout(workout)}` matching parser expectations (title line before steps; use existing title helpers / `ensure_planned_workout_title` output already on the day)

Join days with `\n`. Empty map → empty string (should not happen after validate).

- [ ] **Step 2: Round-trip test**

Parse a multi-day fixture (one rest with reason, one workout with steps), render, parse again; assert dates and rest/workout shape match (not necessarily byte-identical whitespace).

- [ ] **Step 3: Commit** (only if user asked)

---

### Task 2: Use rendered text after corrections (finding 3)

**Files:**
- Modify: `src/domain/training_plan/service/quality/mod.rs` (`generate_quality_replan_draft`)
- Modify: `src/domain/training_plan/service/mod.rs` (initial path before quality loop)

- [ ] **Step 1: Replan**

After successful `apply_correction_rounds` + `validate_snapshot_days`, set:

```rust
let plan_text = self.render_plan_window(&days_by_date);
Ok(QualityReplanDraft {
    snapshot,
    draft: QualityDraft {
        plan_text,
        description: raw_plan_description, // envelope; not correction description
    },
})
```

Checklist check (`draft_addresses_quality_checklist`) must run on this rendered text (Adjustment rules may live only in corrected days — if checklist text was only in pre-correction LLM output and corrections drop it, that is a real bug to surface; prefer rendering from final days).

- [ ] **Step 2: Initial path**

After structural corrections and before `run_plan_quality_loop`, replace `draft.plan_text` / `raw_plan_response` used for the loop with `render_plan_window(&days_by_date)` so the first evaluation sees corrected days.

Keep `operation.raw_plan_response` as originally stored unless finalize/best update rewrites it (Task 3). Optionally also upsert rendered text into `raw_plan_response` after corrections for Mongo = evaluator alignment; **prefer upserting rendered text after corrections on both paths** so operators inspecting the operation see what was graded.

- [ ] **Step 3: Integration coverage**

Extend or add a quality_loop / structural test where correction changes a day and assert the string passed into `evaluate_plan_quality` (fake call log / captured input) contains the corrected day content, not only the pre-correction LLM blob.

---

### Task 3: Persist best description (finding 1)

**Files:**
- Modify: `src/domain/training_plan/model.rs`
- Modify: `src/adapters/mongo/training_plan_generation_operations.rs`
- Modify: `src/domain/training_plan/service/quality/mod.rs` (`BestDraft`, `seed_best_quality_draft`, `finalize_plan_quality_loop`)
- Modify: `src/domain/training_plan/service/quality/attempt.rs`
- Modify: test fixtures under `tests/training_plan_service/support/fixtures.rs`

- [ ] **Step 1: Domain + Mongo field**

Add `best_quality_plan_description: Option<String>` next to `best_quality_plan_response`. Map in mongo document ↔ domain. Default `None` in constructors/fixtures.

- [ ] **Step 2: `with_quality_evaluation`**

Change signature to accept best draft as a pair, e.g.:

```rust
pub fn with_quality_evaluation(
    &self,
    evaluation: PlanQualityEvaluation,
    best_draft: Option<QualityDraftPair>, // plan_text + description
    recorded_at_epoch_seconds: i64,
) -> Self
```

or two `Option`s written atomically in the same `if let`. Never update plan response without description slot (description may be `None` if model omitted it — store `None` explicitly when promoting best).

- [ ] **Step 3: `BestDraft` + attempt path**

```rust
struct BestDraft {
    snapshot: TrainingPlanSnapshot,
    evaluation: PlanQualityEvaluation,
    description: Option<String>,
}
```

On `is_better`, pass `draft.plan_text` + `draft.description.clone()`.

- [ ] **Step 4: Seed / resume**

`seed_best_quality_draft` loads `best_quality_plan_description`. Incomplete state = evaluation present but plan missing (unchanged). Description alone missing is allowed (`None`).

When resuming the loop, after seed, if continuing evaluations, ensure in-loop `QualityDraft.description` is refreshed from best when the current draft is replaced by seeded best snapshot equality path — today seed only affects `best`, not `draft`; if `accepted` early because best already passes, no further eval. If not accepted, first attempt still grades **current** draft (correct). No change required unless reclaim resets draft to best — verify reclaim/resume path and align description if draft is reset to best plan text.

- [ ] **Step 5: Finalize raw-field alignment**

When shipping `best`, if `operation.raw_plan_response` / `raw_plan_description` are not the best pair, upsert via existing payload helper (or a narrow `with_shipped_quality_draft`) so Mongo raw fields match shipped best. Clears accidental last-failed-replan commentary.

- [ ] **Step 6: Tests**

- Model unit: promoting best writes both fields; non-better evaluation does not clobber best description.
- Quality loop: score improves then replan fails validation → shipped snapshot is best; operation `best_quality_plan_description` matches the best attempt’s description; raw fields match best after finalize.
- Mongo round-trip of the new field (`tests/training_plan_mongo.rs` if that suite maps operations).

---

### Task 4: Verification

Sequential on this host:

```bash
export CARGO_BUILD_JOBS=1
cargo test -j 1 --lib training_plan::service -- --test-threads=1
cargo test -j 1 --test training_plan_service quality_loop -- --test-threads=1
cargo test -j 1 --test training_plan_mongo -- --test-threads=1
bun run verify:arch
cargo fmt --all --check
cargo clippy -j 1 --all-targets --all-features -- -D warnings
```

---

## Acceptance

1. After a correction that changes day D, evaluator `draft_plan_text` contains the corrected content for D.
2. Promoting a better quality score persists plan text **and** description together.
3. Replan failure after a better score ships best snapshot; `best_quality_plan_description` and finalized raw description match that best attempt, not the failed replan.

## Self-check

| Finding | Task |
|---------|------|
| (1) best commentary skew | Task 3 |
| (3) pre-correction plan text graded | Task 1–2 |
| Initial path same bug | Task 2 step 2 |
| Correction description misuse | Locked decision 3 (avoided) |
