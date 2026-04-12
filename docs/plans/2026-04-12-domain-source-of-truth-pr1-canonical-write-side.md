# Domain Source of Truth PR1: Canonical Write Side Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce canonical domain write models and provider-agnostic sync metadata without changing current calendar or training-context read paths.

**Architecture:** Add provider-agnostic write-side roots for planned workouts, completed workouts, races, and special days, plus external observation and polling state records. Keep existing readers intact for this PR; this PR only creates the new source-of-truth foundation and removes provider-specific sync data from domain roots such as `Race`.

**Tech Stack:** Rust 2021, MongoDB, Axum app wiring in `src/main.rs`, existing hexagonal ports/adapters, existing Intervals activity persistence reused as the starting backing model for completed workouts.

---

### Task 1: Create the target-model mapping note inside the plan branch

**Files:**
- Modify: `docs/plans/2026-04-12-domain-source-of-truth-pr1-canonical-write-side.md`

**Step 1: Add a short current-to-target mapping section**
Include:
- `Activity -> CompletedWorkout backing model`
- `TrainingPlanSnapshot / TrainingPlanProjectedDay -> source for PlannedWorkout migration`
- `Race -> keep as root, remove provider sync fields`
- special Intervals calendar events -> `SpecialDay`

**Step 2: Add explicit anti-goals**
Include:
- no reader migration in this PR
- no inline provider fetch in read path
- no second completed-workout store beside current activities persistence

**Step 3: Review mapping against current code**
Check:
- `src/domain/races/model.rs`
- `src/domain/training_plan/model.rs`
- `src/adapters/mongo/activities.rs`

**Step 4: Commit**
```bash
git add docs/plans/2026-04-12-domain-source-of-truth-pr1-canonical-write-side.md
git commit -m "docs: define canonical write-side scope for pr1"
```

### Task 2: Add canonical domain modules

**Files:**
- Create: `src/domain/planned_workouts/mod.rs`
- Create: `src/domain/planned_workouts/model.rs`
- Create: `src/domain/planned_workouts/ports.rs`
- Create: `src/domain/completed_workouts/mod.rs`
- Create: `src/domain/completed_workouts/model.rs`
- Create: `src/domain/completed_workouts/ports.rs`
- Create: `src/domain/special_days/mod.rs`
- Create: `src/domain/special_days/model.rs`
- Create: `src/domain/special_days/ports.rs`
- Create: `src/domain/external_sync/mod.rs`
- Create: `src/domain/external_sync/model.rs`
- Create: `src/domain/external_sync/ports.rs`
- Modify: `src/domain/mod.rs`

**Step 1: Write failing compile tests or unit tests**
Add tests that require:
- local canonical IDs on each root
- no provider-specific ID fields on roots
- root models compile and expose minimal constructors

**Step 2: Define minimal write-side models**
Add:
- `PlannedWorkout`
- `CompletedWorkout`
- `SpecialDay`
- shared error enums only if needed
Keep them minimal and provider-agnostic.

**Step 3: Define repository ports**
Add ports for:
- planned workouts
- completed workouts
- special days
- external observations
- external sync state
- provider poll state

**Step 4: Wire modules into `src/domain/mod.rs`**
Export new modules without removing old ones yet.

**Step 5: Run targeted tests**
Run:
```bash
cargo test planned_workouts -- --nocapture
cargo test completed_workouts -- --nocapture
cargo test special_days -- --nocapture
cargo test external_sync -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain
git commit -m "feat: add canonical write-side domain modules"
```

### Task 3: Add provider-agnostic external sync models

**Files:**
- Modify: `src/domain/external_sync/model.rs`
- Modify: `src/domain/external_sync/ports.rs`
- Create: `src/domain/external_sync/tests.rs`
- Modify: `src/domain/external_sync/mod.rs`

**Step 1: Write failing tests for sync semantics**
Cover:
- one canonical entity may have multiple external observations
- same provider and external ID resolves directly
- roundtrip echo is not a conflict
- payload divergence after sync becomes conflict candidate

**Step 2: Define models**
Add:
- `ExternalProvider`
- `ExternalObjectKind`
- `CanonicalEntityRef`
- `ExternalObservation`
- `ExternalSyncState`
- `ProviderPollState`
- `ConflictStatus`

**Step 3: Define key fields**
Include:
- `provider`
- `external_id`
- `canonical_entity_id`
- `last_synced_payload_hash`
- `last_seen_remote_payload_hash`
- `cursor`
- `next_due_at`

**Step 4: Make conflict rules explicit in tests**
Use tests to assert:
- echo after our own push updates sync metadata only
- remote divergence marks conflict without overwriting canonical data

**Step 5: Run tests**
```bash
cargo test external_sync -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain/external_sync
git commit -m "feat: add provider-agnostic sync metadata model"
```

### Task 4: Refactor `Race` to remove Intervals-specific sync fields

**Files:**
- Modify: `src/domain/races/model.rs`
- Modify: `src/domain/races/service.rs`
- Modify: `src/domain/races/ports.rs`
- Modify: `src/domain/races/tests.rs`
- Modify: any Mongo race adapter file used by current service
- Likely modify: `src/adapters/mongo/races.rs`

**Step 1: Write failing tests**
Cover:
- race business fields still behave the same
- race no longer stores provider-specific sync state directly

**Step 2: Remove provider-specific fields from `Race`**
Move out fields such as:
- linked external event IDs
- synced payload hash
- last sync timestamps
- sync status if it is purely provider workflow state

**Step 3: Replace service logic with external-sync dependency**
Where sync workflow still needs state, read or write it through `external_sync` ports.

**Step 4: Update Mongo mapping**
Persist only canonical race fields in the race collection.

**Step 5: Run tests**
```bash
cargo test races -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain/races src/adapters/mongo/races.rs
git commit -m "refactor: move race sync metadata to external sync layer"
```

### Task 5: Add Mongo repositories for sync metadata

**Files:**
- Create: `src/adapters/mongo/external_observations.rs`
- Create: `src/adapters/mongo/external_sync_states.rs`
- Create: `src/adapters/mongo/provider_poll_states.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing repository tests**
Cover:
- unique provider plus external ID index
- many observations pointing to one canonical entity
- per-user scoping
- due polling queries by `next_due_at`

**Step 2: Implement Mongo document models**
Keep document models separate from domain models.

**Step 3: Add indexes**
At minimum:
- `user_id + provider + external_id`
- `user_id + canonical_entity_id`
- `user_id + provider + stream + next_due_at`

**Step 4: Wire repositories into `main.rs`**
Construct repos and keep them available for later PRs.

**Step 5: Run tests**
```bash
cargo test external_observations -- --nocapture
cargo test provider_poll_states -- --nocapture
```

**Step 6: Commit**
```bash
git add src/adapters/mongo src/main.rs
git commit -m "feat: add mongo repositories for sync metadata"
```

### Task 6: Final verification for PR1

**Files:**
- Modify: any files touched above

**Step 1: Run formatter check**
```bash
cargo fmt --all --check
```

**Step 2: Run clippy**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run targeted domain and mongo tests**
```bash
cargo test planned_workouts completed_workouts special_days external_sync races -- --nocapture
```

**Step 4: Run full backend tests if targeted tests are clean**
```bash
cargo test
```

**Step 5: Commit any final fixes**
```bash
git add .
git commit -m "test: stabilize canonical write-side foundation"
```
