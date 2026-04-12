# Domain Source of Truth PR4: Reader Migration And Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate application readers to the persisted domain-backed read model and remove the old Intervals-first event flow from business reads.

**Architecture:** Switch calendar, labels, training context, and related consumer flows to read only from local canonical state and `CalendarEntryView`. Remove or drastically shrink the old `domain::intervals::Event` read path and expose canonical local IDs through APIs where needed. No consumer in this PR may read metrics, streams, intervals, TSS, IF, VI, or workout definitions directly from provider APIs once those fields are persisted canonically in local domain storage.

**Tech Stack:** Rust 2021, Axum REST handlers, persisted Mongo read model, existing training-context module, existing calendar and labels services.

## Boundary Rule

- Reader migrations may depend only on canonical domain roots, canonical value objects, and persisted read models.
- During cleanup, remove any remaining dependency where business read paths import provider-specific domain types from `src/domain/intervals/**` only to avoid modeling the data canonically.
- If a read path still needs provider-shaped data, treat that as a modeling gap in canonical domain storage and fix the model rather than reusing provider types.

---

### Task 1: Migrate calendar domain service to `CalendarEntryView`

**Files:**
- Modify: `src/domain/calendar/model.rs`
- Modify: `src/domain/calendar/ports.rs`
- Modify: `src/domain/calendar/service.rs`
- Modify: tests under `src/domain/calendar/**`

**Step 1: Write failing tests**
Cover:
- calendar list returns entries from local read model only
- planned entries, completed entries, races, and special days all render correctly
- no provider fetch is needed to serve a list request

**Step 2: Remove `domain::intervals::Event` from calendar model**
Replace embedded upstream event data with local read-model shape.

**Step 3: Change service implementation**
Read from `CalendarEntryViewRepository` instead of `IntervalsUseCases`.

**Step 4: Keep command paths separate**
If `sync_planned_workout` remains, it should operate through write side and provider sync state, not through upstream event list reads.

**Step 5: Run tests**
```bash
cargo test calendar -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain/calendar
git commit -m "refactor: read calendar from persisted calendar view"
```

### Task 2: Migrate REST calendar adapter and DTOs to canonical IDs

**Files:**
- Modify: `src/adapters/rest/calendar/dto.rs`
- Modify: `src/adapters/rest/calendar/mapping.rs`
- Modify: `src/adapters/rest/calendar/handlers.rs`
- Modify: tests covering calendar REST behavior

**Step 1: Write failing tests**
Cover:
- canonical local IDs in responses
- correct mixed entry rendering
- no implicit dependency on Intervals event IDs

**Step 2: Update DTOs and mappings**
Keep provider refs as metadata only if needed.

Do not make REST responses dependent on provider fetches for detailed workout data. If detailed metrics or structure are exposed, they must come from canonical local storage.

**Step 3: Update handlers**
Ensure handlers only call local calendar read use cases.

**Step 4: Run tests**
```bash
cargo test calendar_rest -- --nocapture
```

**Step 5: Commit**
```bash
git add src/adapters/rest/calendar tests
git commit -m "refactor: expose canonical calendar ids in rest adapter"
```

### Task 3: Migrate calendar labels to local read model

**Files:**
- Modify: `src/domain/calendar_labels/model.rs`
- Modify: `src/domain/calendar_labels/ports.rs`
- Modify: `src/domain/calendar_labels/service.rs`
- Modify: tests under `src/domain/calendar_labels/**`

**Step 1: Write failing tests**
Cover:
- labels come from local read side
- race labels still render correctly
- planned and special day labels do not require Intervals reads

**Step 2: Switch label source**
Read from `CalendarEntryView` and canonical roots where needed.

**Step 3: Remove Intervals-only assumptions**
Especially around race and special event labeling.

**Step 4: Run tests**
```bash
cargo test calendar_labels -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/calendar_labels
git commit -m "refactor: build calendar labels from local calendar view"
```

### Task 4: Migrate training context to local sources

**Files:**
- Modify: `src/domain/training_context/service/mod.rs`
- Modify: `src/domain/training_context/service/context.rs`
- Modify: `src/domain/training_context/model.rs` if needed
- Modify: tests under `src/domain/training_context/**`

**Step 1: Write failing tests**
Cover:
- recent and upcoming context builds from local read or write data
- sickness or special day context comes from `SpecialDay`
- future planned context comes from local planned workouts or calendar view
- completed workouts use local completed-workout backing data

**Step 2: Replace Intervals event reads**
Remove dependence on:
- `list_events`
- `get_event`
for business reads.

**Step 3: Keep execution metrics from canonical completed-workout data**
Migrate current activity metrics and details into canonical completed-workout reads where still useful, but do not treat provider fetches as the source for those fields.

**Step 4: Rework plan-completion matching**
Use canonical relations where available, fallback heuristics only where necessary.

**Step 5: Run tests**
```bash
cargo test training_context -- --nocapture
```

**Step 6: Commit**
```bash
git add src/domain/training_context
git commit -m "refactor: build training context from local domain state"
```

### Task 5: Migrate any LLM-facing context builders to local source of truth

**Files:**
- Modify: any domain modules producing LLM input from calendar or history state
- Modify: relevant tests

**Step 1: Write failing tests**
Cover:
- LLM context does not need direct provider reads
- calendar and history data come from canonical domain state

**Step 2: Replace upstream event dependencies**
Use local read model or canonical roots only.

If LLM inputs need stream-derived or metric-derived details, source them from canonical persisted domain models rather than provider APIs.

**Step 3: Run tests**
```bash
cargo test llm training_context -- --nocapture
```

**Step 4: Commit**
```bash
git add src/domain
git commit -m "refactor: source llm context from local domain state"
```

### Task 6: Remove legacy Intervals-first read flow

**Files:**
- Modify or remove: `src/domain/intervals/service/events.rs`
- Modify: `src/domain/intervals/service.rs`
- Modify: `src/domain/intervals/mod.rs`
- Modify: `src/main.rs`
- Modify: any tests still targeting legacy read path

**Step 1: Write failing compile checks or grep-based checklist**
Confirm no business reader still depends on legacy Intervals event reads.

**Step 2: Remove unused event-centric APIs**
Delete or shrink:
- legacy list or get event reader methods
- legacy adapters only used by old read flow

**Step 3: Keep only adapter-valid responsibilities**
Retain only code that still makes sense as:
- provider DTO handling
- provider HTTP client logic
- adapter-specific mapping helpers
- shared utilities if truly needed

**Step 4: Run tests**
```bash
cargo test -- --nocapture
```

**Step 5: Commit**
```bash
git add src/domain/intervals src/main.rs
git commit -m "cleanup: remove legacy intervals-first read flow"
```

### Task 7: Final verification for PR4

**Step 1: Run formatter check**
```bash
cargo fmt --all --check
```

**Step 2: Run clippy**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run full backend tests**
```bash
cargo test
```

**Step 4: Run repo Rust verification**
```bash
bun run verify:rust
```

**Step 5: Run frontend tests only if DTO or API contracts changed**
```bash
bun run --cwd frontend test
bun run --cwd frontend build
```

**Step 6: Rebuild graphify as required by repo instructions after implementation session**
Run the repo-required rebuild command after code changes are complete.

**Step 7: Commit final fixes**
```bash
git add .
git commit -m "test: verify local domain as application source of truth"
```
