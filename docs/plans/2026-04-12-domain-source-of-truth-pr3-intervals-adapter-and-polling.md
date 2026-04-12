# Domain Source of Truth PR3: Intervals Adapter And Simple Polling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Convert Intervals from a read-source domain model into a pure provider adapter that imports into canonical write models and updates the persisted read side through simple polling.

**Architecture:** Keep Intervals-specific DTOs and HTTP logic inside the adapter, normalize remote payloads into provider-agnostic import commands, persist external observations and sync state, and run incremental provider polling with persisted cursors. Imported payloads must be transformed into canonical domain data before any consumer can use them; metrics, streams, intervals, TSS, IF, VI, and workout definitions must never be sourced directly from provider reads once canonical persistence exists. Do not migrate calendar or training-context readers in this PR.

**Tech Stack:** Rust 2021, reqwest-based Intervals adapter, MongoDB poll-state persistence, `tokio::time::interval` for simple in-process polling.

## Boundary Rule

- Provider adapters may translate provider DTOs into canonical commands and canonical value objects only at the adapter boundary.
- Do not reuse `src/domain/intervals/**` business types as canonical domain types for completed workouts, planned workouts, races, or special days.
- If a provider exposes a rich model that looks useful, copy the semantics into canonical types and map across explicitly; do not couple canonical domain modules back to provider-specific slices.

---

### Task 1: Add provider-agnostic import service

**Files:**
- Create: `src/domain/external_sync/import_service.rs`
- Modify: `src/domain/external_sync/mod.rs`
- Create: tests for import behavior

**Step 1: Write failing tests for import outcomes**
Cover:
- imported planned event creates or updates planned workout
- imported completed workout creates or updates completed workout
- imported race creates or updates race
- imported special event creates or updates special day

**Step 2: Define provider-agnostic import commands**
Model normalized commands separate from Intervals DTOs.

**Step 3: Implement import orchestration**
Flow:
- normalize payload
- resolve canonical target
- attach or create `ExternalObservation`
- update canonical root
- update `ExternalSyncState`
- trigger `CalendarEntryView` update

When updating canonical roots, persist the full useful payload, including workout structure for planned workouts and metrics or details for completed workouts, rather than only provider references or summary fields.

**Step 4: Run tests**
```bash
cargo test import_service -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/external_sync
git commit -m "feat: add provider-agnostic external import service"
```

### Task 2: Map Intervals payloads into import commands

**Files:**
- Modify: `src/adapters/intervals_icu/**`
- Likely create: mapping helpers inside `src/adapters/intervals_icu/`
- Create: adapter-focused tests

**Step 1: Write failing tests for mapping**
Cover:
- workout-like event
- race-like event
- special-day event
- activity or completed workout payload

**Step 2: Keep Intervals-specific DTOs inside adapter**
Do not leak them into domain modules.

**Step 3: Implement normalization mapping**
Map Intervals data into provider-agnostic commands.

**Step 4: Keep read-side logic out**
No reader should consume Intervals DTOs directly after this mapping layer.

**Step 5: Run tests**
```bash
cargo test intervals_icu -- --nocapture
```

**Step 6: Commit**
```bash
git add src/adapters/intervals_icu
git commit -m "refactor: map intervals payloads into canonical import commands"
```

### Task 3: Add completed-workout cross-provider dedup

**Files:**
- Create: `src/domain/completed_workouts/dedup.rs`
- Create: tests
- Modify: `src/domain/external_sync/import_service.rs`

**Step 1: Write failing tests**
Cover:
- same completed workout imported from Intervals and Wahoo-like provider maps to one canonical workout
- same import order reversed still maps to one canonical workout
- unknown external IDs but matching fingerprint still attach correctly
- ambiguous match does not silently merge

**Step 2: Implement ordered dedup rules**
Use:
- existing direct external ref
- imported cross-system IDs if present
- fallback fingerprint
- explicit ambiguity handling

**Step 3: Reuse current activity dedup logic where valid**
Reference:
- `src/domain/intervals/model.rs`
- `src/adapters/mongo/activities.rs`

Preserve the rich completed-workout payload during dedup. Dedup may choose the canonical entity, but it must not collapse the entity into a summary-only shape.

**Step 4: Run tests**
```bash
cargo test completed_workouts -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/completed_workouts src/domain/external_sync
git commit -m "feat: add cross-provider completed workout dedup"
```

### Task 4: Add polling state query and scheduling loop

**Files:**
- Modify: `src/domain/external_sync/ports.rs`
- Modify: Mongo poll-state repo from PR1
- Create: backend polling module, likely `src/config/provider_polling.rs` or similar
- Modify: `src/main.rs`

**Step 1: Write failing tests for due polling selection**
Cover:
- provider stream due now
- provider stream not due
- updating `last_attempted` and `last_successful`
- handling polling failure and backoff-ready state

**Step 2: Add polling service interface**
Support:
- list due streams
- claim or mark running if needed
- update cursor and timestamps

**Step 3: Add simple in-process polling loop**
Use `tokio::time::interval`.
Keep it single-process friendly and simple.

**Step 4: Wire loop in `main.rs`**
Start background task only when app boots normally.

**Step 5: Run tests**
```bash
cargo test provider_poll -- --nocapture
```

**Step 6: Commit**
```bash
git add src/main.rs src/config src/domain/external_sync src/adapters/mongo
git commit -m "feat: add simple provider polling loop"
```

### Task 5: Implement Intervals backfill and incremental polling

**Files:**
- Modify: Intervals adapter and import service
- Modify: poll-state logic
- Create: tests

**Step 1: Write failing tests**
Cover:
- first sync triggers initial backfill
- later sync uses cursor or watermark
- planned and completed streams can advance independently

**Step 2: Define streams**
At minimum:
- completed workouts
- calendar-like entries for planned, race, special

**Step 3: Implement backfill behavior**
Prefer:
- recent history
- near future
- avoid huge full-history fetch by default

**Step 4: Implement incremental polling**
Advance cursor only after successful import.

**Step 5: Run tests**
```bash
cargo test polling -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain/external_sync src/adapters/intervals_icu src/main.rs
git commit -m "feat: add intervals backfill and incremental polling"
```

### Task 6: Mark streams due after local provider push

**Files:**
- Modify: planned-workout sync flows
- Modify: race sync flows if still present
- Modify: `src/domain/external_sync/*`

**Step 1: Write failing tests**
Cover:
- successful local push marks related provider stream due soon
- no forced inline read-after-write fetch
- later poll confirms echo without conflict

**Step 2: Implement due-soon scheduling**
Update `ProviderPollState.next_due_at` after push success.

**Step 3: Keep read path untouched**
Do not fetch provider inline from calendar or training-context APIs.

**Step 4: Run tests**
```bash
cargo test sync_due_soon -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain src/adapters
git commit -m "feat: mark provider streams due after local sync"
```

### Task 7: Final verification for PR3

**Step 1: Run formatter check**
```bash
cargo fmt --all --check
```

**Step 2: Run clippy**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run targeted sync tests**
```bash
cargo test external_sync completed_workouts intervals_icu -- --nocapture
```

**Step 4: Run full backend tests**
```bash
cargo test
```

**Step 5: Commit final fixes**
```bash
git add .
git commit -m "test: stabilize intervals adapter import and polling"
```
