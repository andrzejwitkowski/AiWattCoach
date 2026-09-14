# Settings megamodule split + bounded int knobs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the two worst maintainability regressions from the plan-quality work: split the oversized Mongo settings adapter and settings REST suite, then collapse duplicated “bounded optional int” settings plumbing so new knobs stop cloning across ten files.

**Architecture:** Mirror the existing `src/adapters/mongo/workout_summary/` directory layout (`document` / `mapping` / `repository` / `tests` + thin `mod.rs`). Keep public API stable (`MongoUserSettingsRepository`, `IntervalsPollBootstrapUser`). For REST tests, extract AI-agents coverage (including plan-quality) into a sibling file under the existing `tests/settings_rest/` suite. For knobs, add one domain helper for 1..=10 optional ints and thin frontend helpers already started by deslop; stop adding twin validators/ports without that shared path.

**Tech Stack:** Rust 2021, Mongo adapter, Axum settings REST tests, Bun/Vitest frontend settings

**Out of scope (explicit deferrals from thermo):**
- Quality-loop state-machine rewrite in `service/quality.rs`
- Moving progress alias off `SaveWorkflowCompletionPort` onto a dedicated progress-routing port
- Full rewrite of frontend draft field tables into a generic schema-driven form

**Commits:** Do not commit unless the user explicitly asks.

**Host constraint:** `CARGO_BUILD_JOBS=1`; one `cargo` invocation at a time.

---

## File map

| Area | Create | Modify | Delete |
| --- | --- | --- | --- |
| Mongo settings directory | `src/adapters/mongo/settings/mod.rs`, `document.rs`, `mapping.rs`, `repository.rs`, `tests.rs` | `src/adapters/mongo/mod.rs` (still `pub mod settings`) | `src/adapters/mongo/settings.rs` |
| Settings REST | `tests/settings_rest/ai_agents_endpoints.rs` | `tests/settings_rest/main.rs`, shrink `settings_endpoints.rs` | — |
| Bounded int domain | — | `src/domain/settings/validation.rs`, `mod.rs` | — |
| LLM config twin methods | — | `src/adapters/llm/workout_llm_config.rs` | — |
| Frontend (light finish) | — | only if Task 4 finds remaining duplicate int plumbing | — |

**Target sizes after split (approximate):**
- each mongo settings sibling **< 600 lines**
- `settings_endpoints.rs` drops by the AI-agents tests moved out (plan-quality + other `update_ai_agents*` / AI-agent GET assertions that belong with them)
- no behavior change

---

### Task 1: Split Mongo settings into a directory module (mechanical)

**Files:**
- Create: `src/adapters/mongo/settings/mod.rs`
- Create: `src/adapters/mongo/settings/document.rs`
- Create: `src/adapters/mongo/settings/mapping.rs`
- Create: `src/adapters/mongo/settings/repository.rs`
- Create: `src/adapters/mongo/settings/tests.rs`
- Delete: `src/adapters/mongo/settings.rs`
- Modify: none needed in `src/adapters/mongo/mod.rs` if `pub mod settings;` already resolves to `settings/mod.rs`

- [ ] **Step 1: Create the directory skeleton without behavior change**

Move code from `src/adapters/mongo/settings.rs` into siblings with these ownership rules:

| File | Owns |
| --- | --- |
| `document.rs` | All `*Document` structs, `IntervalsPollBootstrapUser` (+ bootstrap document structs), `default_availability_*`, `RedactedOptionalText`, `Debug` impls for redacted docs |
| `mapping.rs` | `map_document_to_domain`, `map_domain_to_document`, cycling/availability mappers/repair helpers |
| `repository.rs` | `MongoUserSettingsRepository`, `impl` constructors/index helpers, `impl UserSettingsRepository`, bootstrap query helpers used only by repository |
| `tests.rs` | Entire former `#[cfg(test)] mod tests` body |
| `mod.rs` | `mod document; mod mapping; mod repository;` + `#[cfg(test)] mod tests;` + re-exports |

`mod.rs` must keep the public surface identical:

```rust
mod document;
mod mapping;
mod repository;

#[cfg(test)]
mod tests;

pub use document::IntervalsPollBootstrapUser;
pub use repository::MongoUserSettingsRepository;
```

Visibility rules while splitting:
- Document types used by mapping/repository/tests: `pub(super)` or `pub(crate)` as needed inside the `settings` module.
- Mapping fns: `pub(super)`.
- Do **not** change method signatures on `MongoUserSettingsRepository` or `UserSettingsRepository`.

- [ ] **Step 2: Delete the old monolithic file**

After the directory compiles, remove `src/adapters/mongo/settings.rs` so Rust does not treat both `settings.rs` and `settings/` as the module.

- [ ] **Step 3: Compile and run settings-focused tests**

Run sequentially:

```bash
export CARGO_BUILD_JOBS=1
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib adapters::mongo::settings -- --test-threads=1
```

Expected: clippy clean; settings unit/integration tests in the new `tests` module pass.

- [ ] **Step 4: Confirm line-count targets**

```bash
wc -l src/adapters/mongo/settings/*.rs
```

Expected: no single sibling ≥ 1000 lines; preferably each < 600. If `repository.rs` is still huge, stop and split bootstrap helpers into `bootstrap.rs` before continuing (do not leave another 1k file).

---

### Task 2: Extract AI-agents REST tests into a sibling suite file

**Files:**
- Create: `tests/settings_rest/ai_agents_endpoints.rs`
- Modify: `tests/settings_rest/main.rs`
- Modify: `tests/settings_rest/settings_endpoints.rs` (remove moved tests only)

- [ ] **Step 1: Add the module to the suite**

In `tests/settings_rest/main.rs`:

```rust
mod admin_endpoints;
mod ai_agents_endpoints;
mod athlete_summary_endpoints;
mod intervals_connection;
mod observability;
mod settings_endpoints;
mod shared;
mod tracing_capture;
```

- [ ] **Step 2: Move AI-agents tests**

Move every test whose primary subject is AI-agents PATCH/GET behavior into `ai_agents_endpoints.rs`, including at least:

- `update_ai_agents_saves_and_returns_updated_settings` (and any other `update_ai_agents_*` / provider-key / openai-compatible tests currently in `settings_endpoints.rs`)
- `update_ai_agents_persists_plan_quality_evaluator_override_max_loops_and_pass_score`
- `update_ai_agents_rejects_plan_quality_max_loops_out_of_range`
- `update_ai_agents_rejects_plan_quality_pass_score_out_of_range`
- `test_ai_agents_connection_*` tests if they live in `settings_endpoints.rs`

Keep shared helpers via existing `tests/settings_rest/shared/**`. Prefer `use super::shared::*;` / the same imports the moved tests already use. Do not invent new test harnesses.

File header pattern:

```rust
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use super::shared::*;
```

(Adjust imports to match whatever the moved tests actually need after cut-paste.)

- [ ] **Step 3: Leave non-AI settings tests in place**

`settings_endpoints.rs` should retain intervals/options/availability/cycling/get-settings defaults/auth tests that are not AI-agent focused.

- [ ] **Step 4: Run the settings_rest suite**

```bash
export CARGO_BUILD_JOBS=1
cargo test --test settings_rest -- --test-threads=1
wc -l tests/settings_rest/settings_endpoints.rs tests/settings_rest/ai_agents_endpoints.rs
```

Expected: suite green; `settings_endpoints.rs` materially smaller; AI-agents + plan-quality coverage lives in `ai_agents_endpoints.rs`.

---

### Task 3: Collapse bounded optional int validation in domain settings

**Files:**
- Modify: `src/domain/settings/validation.rs`
- Modify: `src/domain/settings/mod.rs` (exports only if a new public helper is added)

Current state after deslop already has private `validate_plan_quality_range`. Finish the job so both knobs share one **named** policy and defaults stay single-sourced.

- [ ] **Step 1: Make the shared range helper the canonical API**

In `src/domain/settings/validation.rs`, keep thin wrappers but ensure there is exactly one range check and one default source for pass score:

```rust
pub const PLAN_QUALITY_BOUNDED_INT_MIN: u32 = 1;
pub const PLAN_QUALITY_BOUNDED_INT_MAX: u32 = 10;

pub fn validate_plan_quality_bounded_int(
    value: Option<u32>,
    field: &str,
) -> Result<Option<u32>, SettingsError> {
    match value {
        Some(v) if !(PLAN_QUALITY_BOUNDED_INT_MIN..=PLAN_QUALITY_BOUNDED_INT_MAX).contains(&v) => {
            Err(SettingsError::Validation(format!(
                "{field} must be between {PLAN_QUALITY_BOUNDED_INT_MIN} and {PLAN_QUALITY_BOUNDED_INT_MAX}"
            )))
        }
        _ => Ok(value),
    }
}

pub fn validate_plan_quality_max_loops(
    max_loops: Option<u32>,
) -> Result<Option<u32>, SettingsError> {
    validate_plan_quality_bounded_int(max_loops, "planQualityMaxLoops")
}

pub fn validate_plan_quality_pass_score(
    pass_score: Option<u32>,
) -> Result<Option<u32>, SettingsError> {
    validate_plan_quality_bounded_int(pass_score, "planQualityPassScore")
}

pub fn effective_plan_quality_max_loops(max_loops: Option<u32>) -> u32 {
    max_loops.unwrap_or(5)
}

pub fn effective_plan_quality_pass_score(pass_score: Option<u32>) -> u8 {
    pass_score
        .unwrap_or(u32::from(crate::domain::training_plan::PLAN_QUALITY_PASS_SCORE))
        as u8
}
```

Remove any leftover private duplicate `validate_plan_quality_range` if it still exists beside the public helper.

- [ ] **Step 2: Export the shared validator if REST mapping should call it directly**

Prefer keeping REST mapping on the named wrappers (`validate_plan_quality_max_loops` / `_pass_score`) so field names stay obvious at the call site. Only export `validate_plan_quality_bounded_int` from `settings/mod.rs` if a third knob appears in this PR (YAGNI: do not invent a third knob here).

- [ ] **Step 3: Extend unit tests**

In `validation.rs` tests, keep existing max-loops / pass-score cases and add one direct test for the shared helper:

```rust
#[test]
fn plan_quality_bounded_int_rejects_outside_1_to_10() {
    assert_eq!(
        validate_plan_quality_bounded_int(Some(0), "fieldX").unwrap_err(),
        SettingsError::Validation("fieldX must be between 1 and 10".to_string())
    );
    assert_eq!(
        validate_plan_quality_bounded_int(Some(7), "fieldX").unwrap(),
        Some(7)
    );
}
```

- [ ] **Step 4: Verify**

```bash
export CARGO_BUILD_JOBS=1
cargo test --lib plan_quality_ -- --test-threads=1
```

Expected: pass-score + max-loops + bounded-int tests pass.

---

### Task 4: Collapse twin `get_plan_quality_*` loaders in workout LLM config

**Files:**
- Modify: `src/adapters/llm/workout_llm_config.rs`

- [ ] **Step 1: Extract one private loader helper**

Replace the duplicated bodies of `get_plan_quality_max_loops` / `get_plan_quality_pass_score` with:

```rust
fn map_ai_agents<T, F>(
    settings_service: Arc<dyn UserSettingsUseCases>,
    user_id: String,
    map: F,
) -> TrainingPlanBoxFuture<Result<T, TrainingPlanError>>
where
    T: Send + 'static,
    F: FnOnce(AiAgentsConfig) -> T + Send + 'static,
{
    Box::pin(async move {
        let ai_agents = load_ai_agents(&settings_service, &user_id)
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
        Ok(map(ai_agents))
    })
}
```

Then:

```rust
fn get_plan_quality_max_loops(
    &self,
    user_id: &str,
) -> TrainingPlanBoxFuture<Result<u32, TrainingPlanError>> {
    map_ai_agents(self.settings_service.clone(), user_id.to_string(), |ai_agents| {
        effective_plan_quality_max_loops(ai_agents.plan_quality_max_loops)
    })
}

fn get_plan_quality_pass_score(
    &self,
    user_id: &str,
) -> TrainingPlanBoxFuture<Result<u8, TrainingPlanError>> {
    map_ai_agents(self.settings_service.clone(), user_id.to_string(), |ai_agents| {
        effective_plan_quality_pass_score(ai_agents.plan_quality_pass_score)
    })
}
```

Do **not** merge the two port methods into one; the port API staying explicit is fine. Delete only the duplicated load/map boilerplate.

- [ ] **Step 2: Verify adapter + quality loop still compile/test**

```bash
export CARGO_BUILD_JOBS=1
cargo test --test training_plan_service -- --test-threads=1
cargo test --test settings_rest -- --test-threads=1
```

Expected: green.

---

### Task 5: Final verification + graphify

- [ ] **Step 1: Arch + clippy + focused suites**

```bash
export CARGO_BUILD_JOBS=1
bun run verify:arch
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib adapters::mongo::settings -- --test-threads=1
cargo test --test settings_rest -- --test-threads=1
cargo test --test training_plan_service -- --test-threads=1
bun run --cwd frontend test src/features/settings/aiAgentsDraft.test.ts src/features/settings/components/AiAgentsCard.test.tsx
```

Expected: all green.

- [ ] **Step 2: Rebuild graphify after code edits**

```bash
./scripts/rebuild_graphify.sh
```

- [ ] **Step 3: Record the structural win**

```bash
wc -l src/adapters/mongo/settings/*.rs tests/settings_rest/settings_endpoints.rs tests/settings_rest/ai_agents_endpoints.rs
```

Done only when:
- no mongo settings sibling ≥ 1000 lines
- AI-agents/plan-quality REST tests are not in the catch-all `settings_endpoints.rs`
- bounded int validation has one range implementation
- twin settings-load bodies in `workout_llm_config.rs` are gone

---

## Self-review

1. **Spec coverage:** Thermo crucial items covered — mongo split (Task 1), settings_rest split (Task 2), bounded-int collapse (Task 3), twin loader collapse (Task 4). Deferred items listed in Out of scope.
2. **Placeholders:** None; commands and ownership tables are concrete.
3. **Type consistency:** Public types remain `MongoUserSettingsRepository` / `IntervalsPollBootstrapUser`; port methods stay `get_plan_quality_max_loops` / `get_plan_quality_pass_score`.
