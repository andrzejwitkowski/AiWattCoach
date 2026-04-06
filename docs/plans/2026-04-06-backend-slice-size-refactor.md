# Backend Slice Size Refactor Implementation Plan

**Goal:** Split oversized files in the current training-plan and workout-summary backend slice into smaller concern-based modules without changing behavior.

**Architecture:** Convert oversized single-file modules into directory modules with focused siblings for production logic, mapping/filter helpers, and tests. Keep public exports stable and preserve persist-before-side-effects ordering, idempotency behavior, and current adapter boundaries.

**Tech Stack:** Rust, MongoDB, cargo test, cargo fmt, cargo clippy

---

### Task 1: Split `training_context` service into focused modules

**Files:**
- Modify: `src/domain/training_context/mod.rs`
- Replace: `src/domain/training_context/service.rs`
- Create: `src/domain/training_context/service/mod.rs`
- Create: focused siblings under `src/domain/training_context/service/`

**Step 1: Write the failing compile target through the move**

Move the existing file into a directory-based module layout without changing logic yet. Let the compiler show missing imports or visibility problems.

**Step 2: Split by concern**

Keep the root module small and move helpers into focused siblings for:
- build orchestration
- context assembly helpers
- power compression helpers
- date/load helpers
- tests

**Step 3: Run focused verification**

Run:
- `cargo test training_context --lib -- --nocapture`
- `cargo test --test llm_adapters -- --nocapture`

Expected: PASS.

### Task 2: Split `workout_summary` service into focused modules

**Files:**
- Modify: `src/domain/workout_summary/mod.rs`
- Replace: `src/domain/workout_summary/service.rs`
- Create: `src/domain/workout_summary/service/mod.rs`
- Create: focused siblings under `src/domain/workout_summary/service/`

**Step 1: Keep `WorkoutSummaryService` as the entrypoint**

Leave constructors and core type definitions in the root module.

**Step 2: Move trait implementation and helpers**

Split out:
- use-case method implementations
- message/reply workflow helpers
- tests and test doubles

**Step 3: Run focused verification**

Run:
- `cargo test --test workout_summary_service -- --nocapture`

Expected: PASS.

### Task 3: Split `training_context` packing into focused modules

**Files:**
- Replace: `src/domain/training_context/packing.rs`
- Create: `src/domain/training_context/packing/mod.rs`
- Create: focused siblings under `src/domain/training_context/packing/`

**Step 1: Separate root API from payload mappers**

Keep `render_training_context()` and `approximate_token_count()` exported from the root module.

**Step 2: Move compact payload structures and tests**

Split payload mappers and test cases into focused sibling files.

**Step 3: Run focused verification**

Run:
- `cargo test training_context --lib -- --nocapture`
- `cargo test --test llm_adapters -- --nocapture`

Expected: PASS.

### Task 4: Split the Mongo workout-summary adapter

**Files:**
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/adapters/mongo/workout_summary.rs`
- Create: `src/adapters/mongo/workout_summary_tests.rs`

**Step 1: Keep repository behavior unchanged**

Move tests out of the main adapter file first if that is enough to drop the main file under the threshold.

**Step 2: Separate helper concerns if still needed**

Split document definitions, mapping helpers, or filters only if the file still remains oversized after moving tests.

**Step 3: Run focused verification**

Run:
- `cargo test --test workout_summary_service -- --nocapture`

Expected: PASS.

### Task 5: Split bulky current-slice test helpers and test suites

**Files:**
- Replace: `tests/workout_summary_service/shared.rs`
- Create: `tests/workout_summary_service/shared/mod.rs`
- Create: focused siblings under `tests/workout_summary_service/shared/`
- Replace: `tests/llm_adapters.rs` if still over threshold after prior refactors
- Replace: `tests/llm_rest/support/in_memory.rs` if still over threshold after prior refactors

**Step 1: Split shared workout-summary helpers first**

Separate repository doubles, coach-reply doubles, athlete-summary doubles, and service builders.

**Step 2: Split remaining oversized current-slice tests if needed**

Use directory-based integration test suites with `main.rs` plus focused siblings.

**Step 3: Run focused verification**

Run:
- `cargo test --test workout_summary_service -- --nocapture`
- `cargo test --test llm_adapters -- --nocapture`

Expected: PASS.

### Task 6: Run broader verification and reviewer passes

**Files:**
- Modify only if verification or review exposes a real issue

**Step 1: Run broader backend verification**

Run:
- `cargo test -- --nocapture`
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

**Step 2: Run reviewer passes**

Run three passes over the resulting diff:
- strict reviewer pass
- very strict reviewer pass
- nitpicker pass

Use focused review context and report findings by severity with file references.
