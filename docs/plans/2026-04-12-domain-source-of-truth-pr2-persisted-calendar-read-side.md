# Domain Source of Truth PR2: Persisted Calendar Read Side Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a persisted calendar read model that becomes the future single source for UI, training context, and LLM inputs.

**Architecture:** Add `CalendarEntryView` as a read model projected from canonical write-side roots. Support fast date-range reads, typed calendar rendering, projection rebuild, and integrity checks against the canonical write side. Do not migrate existing readers yet.

**Tech Stack:** Rust 2021, MongoDB, domain projection services, existing write-side roots from PR1.

---

### Task 1: Add calendar view domain module

**Files:**
- Create: `src/domain/calendar_view/mod.rs`
- Create: `src/domain/calendar_view/model.rs`
- Create: `src/domain/calendar_view/ports.rs`
- Create: `src/domain/calendar_view/service.rs`
- Modify: `src/domain/mod.rs`

**Step 1: Write failing tests for mixed-type calendar view reads**
Cover date-range reads returning:
- planned workout entries
- completed workout entries
- race entries
- special day entries

**Step 2: Define `CalendarEntryView`**
Include fields such as:
- `entry_id`
- `entry_kind`
- `date`
- optional `start_date_local`
- `title`
- `subtitle`
- `description`
- `planned_workout_id`
- `completed_workout_id`
- source or provider summary fields if needed for read side only

**Step 3: Add read-model repository port**
Add query methods by:
- user
- date range
- kind if needed

**Step 4: Run tests**
```bash
cargo test calendar_view -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_view src/domain/mod.rs
git commit -m "feat: add calendar entry view domain module"
```

### Task 2: Add Mongo repository for calendar entry views

**Files:**
- Create: `src/adapters/mongo/calendar_entry_views.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing repository tests**
Cover:
- date-range reads
- sorting by date
- mixed entry kinds
- per-user scoping

**Step 2: Implement document mapping**
Keep Mongo document separate from domain view.

**Step 3: Add indexes**
At minimum:
- `user_id + date`
- `user_id + entry_kind + date`
- `user_id + entry_id`

**Step 4: Wire repository into `main.rs`**
Do not change existing readers yet.

**Step 5: Run tests**
```bash
cargo test calendar_entry_views -- --nocapture
```

**Step 6: Commit**
```bash
git add src/adapters/mongo src/main.rs
git commit -m "feat: add mongo repository for calendar entry views"
```

### Task 3: Add projectors from canonical roots to calendar view

**Files:**
- Create: `src/domain/calendar_view/projection.rs`
- Create: `src/domain/calendar_view/tests.rs`
- Modify: `src/domain/calendar_view/mod.rs`
- Modify: `src/domain/calendar_view/service.rs`

**Step 1: Write failing tests for each projector**
Cover:
- `PlannedWorkout -> CalendarEntryView`
- `CompletedWorkout -> CalendarEntryView`
- `Race -> CalendarEntryView`
- `SpecialDay -> CalendarEntryView`

**Step 2: Implement projector helpers**
Create deterministic mapping helpers for each root.

**Step 3: Make view semantics explicit**
Ensure:
- one planned workout produces one planned entry
- one completed workout produces one completed entry
- race and special days keep their labels meaningful

**Step 4: Run tests**
```bash
cargo test calendar_view::tests -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_view
git commit -m "feat: add projectors for persisted calendar view"
```

### Task 4: Add rebuild support for the read model

**Files:**
- Create: `src/domain/calendar_view/rebuild.rs`
- Modify: `src/domain/calendar_view/service.rs`
- Create: tests in `src/domain/calendar_view/tests.rs` or separate file

**Step 1: Write failing tests for rebuild**
Cover:
- rebuild from empty view store
- rebuild after stale entries
- rebuild preserves all canonical roots

**Step 2: Implement rebuild flow**
Read canonical roots and replace the view store deterministically.

**Step 3: Keep rebuild idempotent**
Test repeated rebuilds produce same result.

**Step 4: Run tests**
```bash
cargo test rebuild -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_view
git commit -m "feat: add calendar view rebuild support"
```

### Task 5: Add read or write integrity checks

**Files:**
- Create: `src/domain/calendar_view/integrity.rs`
- Modify: `src/domain/calendar_view/mod.rs`
- Create: tests

**Step 1: Write failing tests**
Cover:
- missing view rows for canonical entities
- duplicate view rows
- type mismatches
- stale orphaned rows

**Step 2: Implement integrity helpers**
Return explicit mismatches that are easy to inspect.

**Step 3: Keep checks query-friendly**
Design results so debugging in Mongo stays easy.

**Step 4: Run tests**
```bash
cargo test integrity -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_view
git commit -m "feat: add calendar view integrity checks"
```

### Task 6: Wire projection updates from write-side changes

**Files:**
- Modify: relevant services touching write-side roots
- Modify: `src/main.rs`
- Modify: `src/domain/calendar_view/service.rs`

**Step 1: Write failing tests**
Cover:
- create planned workout updates view
- create completed workout updates view
- create or update race updates view
- create special day updates view

**Step 2: Implement synchronous projector updates**
Update view in the same write flow where practical.

**Step 3: Avoid async queue complexity**
Keep projection updates simple in this PR.

**Step 4: Run tests**
```bash
cargo test calendar_view -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_view src/main.rs src/domain
git commit -m "feat: update calendar view synchronously from write side"
```

### Task 7: Final verification for PR2

**Step 1: Run formatter check**
```bash
cargo fmt --all --check
```

**Step 2: Run clippy**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run targeted tests**
```bash
cargo test calendar_view -- --nocapture
```

**Step 4: Run full backend tests**
```bash
cargo test
```

**Step 5: Commit final fixes**
```bash
git add .
git commit -m "test: stabilize persisted calendar read model"
```
