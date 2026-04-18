# Training Load History Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist 2 years of completed workouts, introduce durable FTP history plus daily training-load snapshots, and switch `training_context` to consume domain-derived historical metrics that are correct against FTP changes over time.

**Architecture:** `completed_workouts` remains the durable workout source of truth, `ftp_history` becomes the durable source of effective FTP by date, and `training_load_daily_snapshots` becomes a read model for charts and LLM context. Recompute runs in batch after completed-workout polling and after FTP changes in settings, so we avoid recalculating 2 years of history once per imported workout.

**Tech Stack:** Rust, Axum, MongoDB, chrono, existing hexagonal ports/adapters, existing LLM context builder

---

### Task 1: Add training load domain models and ports

**Files:**
- Create: `src/domain/training_load/mod.rs`
- Create: `src/domain/training_load/model.rs`
- Create: `src/domain/training_load/ports.rs`
- Create: `src/domain/training_load/tests.rs`
- Modify: `src/domain/mod.rs`

**Steps:**
1. Add failing domain tests for `FtpHistoryEntry`, `TrainingLoadDailySnapshot`, and repository traits.
2. Run `cargo test training_load -- --nocapture` and confirm failure.
3. Implement minimal domain models and ports.
4. Run `cargo test training_load -- --nocapture` and confirm pass.

### Task 2: Add Mongo repositories for FTP history and daily snapshots

**Files:**
- Create: `src/adapters/mongo/ftp_history.rs`
- Create: `src/adapters/mongo/training_load_daily_snapshots.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Steps:**
1. Add failing adapter tests for upsert/list/effective lookup semantics.
2. Run targeted cargo tests and confirm failure.
3. Implement Mongo adapters with indexes.
4. Re-run targeted cargo tests and confirm pass.

### Task 3: Implement training-load calculation and recompute workflow

**Files:**
- Create: `src/domain/training_load/service.rs`
- Create: `src/domain/training_load/use_cases.rs`
- Modify: `src/domain/training_load/mod.rs`
- Modify: `src/domain/training_load/tests.rs`

**Steps:**
1. Add failing tests for daily load computation, FTP effective-date selection, and recompute behavior.
2. Run `cargo test training_load -- --nocapture` and confirm failure.
3. Implement minimal calculator and recompute service.
4. Re-run `cargo test training_load -- --nocapture` and confirm pass.

### Task 4: Record FTP history from settings and trigger recompute

**Files:**
- Modify: `src/domain/settings/service.rs`
- Modify: `src/domain/settings/ports.rs` if needed for dependencies
- Modify: `src/main.rs`

**Steps:**
1. Add failing tests for initial FTP-history seed and later FTP changes.
2. Run `cargo test settings::service -- --nocapture` and confirm failure.
3. Persist settings, then seed/update FTP history, then recompute, then invalidate cache.
4. Re-run `cargo test settings::service -- --nocapture` and confirm pass.

### Task 5: Recompute after completed-workout polling and extend bootstrap to 2 years

**Files:**
- Modify: `src/config/provider_polling/mod.rs`
- Modify: `src/config/provider_polling/tests/completed_workouts.rs`
- Modify: `src/main.rs`

**Steps:**
1. Add failing tests for two-year initial range and one recompute per successful batch.
2. Run targeted polling tests and confirm failure.
3. Implement two-year initial range and batch recompute from the earliest imported date.
4. Re-run targeted polling tests and confirm pass.

### Task 6: Switch training context history to daily snapshots

**Files:**
- Modify: `src/domain/training_context/service/mod.rs`
- Modify: `src/domain/training_context/service/context.rs`
- Modify: `src/domain/training_context/service/history.rs`
- Modify: `src/domain/training_context/model.rs`
- Modify: `src/main.rs`

**Steps:**
1. Add failing tests proving `CTL`, `ATL`, `TSB`, averages, and FTP change come from snapshots/history.
2. Run `cargo test training_context -- --nocapture` and confirm failure.
3. Implement snapshot-backed historical context while keeping recent workout rendering intact.
4. Re-run `cargo test training_context -- --nocapture` and confirm pass.

### Task 7: Final verification

**Steps:**
1. Run targeted suites:
   - `cargo test training_load -- --nocapture`
   - `cargo test settings::service -- --nocapture`
   - `cargo test completed_workouts -- --nocapture`
   - `cargo test training_context -- --nocapture`
2. Run repo checks:
   - `bun run verify:arch`
   - `cargo fmt --all --check`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo test`
3. Rebuild graphify with `bash ./scripts/rebuild_graphify.sh`.
