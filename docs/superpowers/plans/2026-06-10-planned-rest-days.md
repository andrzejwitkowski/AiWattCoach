# Planned Rest Days Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship user-managed planned rest days (single day or inclusive ranges) with full CRUD, calendar overlays, and LLM packed context.

**Architecture:** New `planned_rest_days` domain module + Mongo repository + REST adapter; composite calendar label source; training context stable field `prd`; frontend feature module + nav page; calendar label rendering distinct from AI `restDay` events.

**Tech Stack:** Rust 2021, Axum, MongoDB, React, TypeScript, Zod, Vitest, i18next

**Design spec:** `docs/superpowers/specs/2026-06-10-planned-rest-days-design.md`

---

## File map

| Area | Create | Modify |
| --- | --- | --- |
| Domain | `src/domain/planned_rest_days/{mod,model,ports,service,error}.rs`, `tests.rs` | `src/domain/mod.rs`, `src/domain/calendar_labels/{model,mod}.rs`, `src/domain/calendar_labels/composite.rs`, `src/domain/training_context/{model,mod}.rs`, `src/domain/training_context/packing/payloads/stable.rs`, `src/domain/training_context/service/mod.rs` |
| Mongo | `src/adapters/mongo/planned_rest_days.rs`, `planned_rest_days_calendar.rs` | `src/adapters/mongo/mod.rs` |
| REST | `src/adapters/rest/planned_rest_days/{mod,handlers,dto,mapping,error}.rs` | `src/adapters/rest/mod.rs`, `src/adapters/rest/calendar/{dto,mapping}.rs` |
| Wiring | — | `src/main.rs`, `src/config/app_state.rs` (if new service field) |
| LLM legend | — | `src/domain/llm/context_prelude.rs` |
| Tests | `tests/planned_rest_days_rest/{main,support,crud,labels}.rs` | `tests/Cargo.toml` or workspace test target as needed |
| Frontend feature | `frontend/src/features/planned-rest-days/**` | — |
| Frontend calendar | — | `frontend/src/features/calendar/{types,dayItems,components/CalendarDayCell.tsx,...}` |
| Frontend shell | `frontend/src/pages/PlannedRestDaysPage.tsx` | `frontend/src/App.tsx`, `frontend/src/app/AuthenticatedLayout.tsx`, `frontend/src/i18n.ts` |

---

### Task 1: Domain model + ports

**Files:**
- Create: `src/domain/planned_rest_days/mod.rs`, `model.rs`, `ports.rs`, `error` via model
- Modify: `src/domain/mod.rs`

- [ ] **Step 1: Write failing domain tests**

```rust
// src/domain/planned_rest_days/tests.rs
#[test]
fn new_rejects_end_before_start() {
    let err = PlannedRestDay::new(
        "prd:u:1".into(),
        "user-1".into(),
        "2026-07-10".into(),
        "2026-07-09".into(),
        None,
        None,
        1,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}

#[test]
fn single_day_allowed_when_start_equals_end() {
    let day = PlannedRestDay::new(
        "prd:u:1".into(),
        "user-1".into(),
        "2026-07-10".into(),
        "2026-07-10".into(),
        Some("Recovery".into()),
        None,
        1,
        1,
    )
    .unwrap();
    assert_eq!(day.start_date, day.end_date);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `CARGO_BUILD_JOBS=1 cargo test planned_rest_day::tests -- --nocapture`
Expected: FAIL — module not found

- [ ] **Step 3: Implement model + ports**

```rust
// ports.rs — mirror races
pub trait PlannedRestDayRepository: Send + Sync + 'static {
    fn list_intersecting_range(&self, user_id: &str, range: &DateRange)
        -> BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>>;
    fn find_by_id(&self, user_id: &str, id: &str)
        -> BoxFuture<Result<Option<PlannedRestDay>, PlannedRestDayError>>;
    fn upsert(&self, entry: PlannedRestDay) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;
    fn delete(&self, user_id: &str, id: &str) -> BoxFuture<Result<(), PlannedRestDayError>>;
}

pub trait PlannedRestDayUseCases: Send + Sync {
    fn list(&self, user_id: &str, range: &DateRange)
        -> BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>>;
    fn get(&self, user_id: &str, id: &str)
        -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;
    fn create(&self, user_id: &str, request: CreatePlannedRestDay)
        -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;
    fn update(&self, user_id: &str, id: &str, request: UpdatePlannedRestDay)
        -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;
    fn delete(&self, user_id: &str, id: &str)
        -> BoxFuture<Result<(), PlannedRestDayError>>;
}
```

- [ ] **Step 4: Run tests**

Run: `CARGO_BUILD_JOBS=1 cargo test planned_rest_day -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit** (only when user asks)

---

### Task 2: Domain service

**Files:**
- Create: `src/domain/planned_rest_days/service.rs`
- Modify: `src/domain/planned_rest_days/mod.rs`

- [ ] **Step 1: Write failing service test**

```rust
#[tokio::test]
async fn create_rejects_fully_past_range() {
    let repo = InMemoryPlannedRestDayRepository::default();
    let service = PlannedRestDayService::new(repo, FakeClock::at_date("2026-06-10"), FakeIds);
    let err = service
        .create(
            "user-1",
            CreatePlannedRestDay {
                start_date: "2026-06-01".into(),
                end_date: "2026-06-05".into(),
                title: None,
                note: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PlannedRestDayError::Validation(_)));
}
```

- [ ] **Step 2: Run — expect FAIL**

- [ ] **Step 3: Implement `PlannedRestDayService`**

Key helpers:
- `intersects_range(entry, query)` — `entry.start <= query.newest && entry.end >= query.oldest`
- `validate_write_range(today, start, end)` — end >= today, span <= 366 days
- `expand_dates(start, end) -> Vec<String>` — for calendar label adapter (can live in domain util)

Use `Clock` + `IdGenerator` like `RaceService` but **without** Intervals or external sync.

- [ ] **Step 4: Run service tests — PASS**

---

### Task 3: Mongo repository

**Files:**
- Create: `src/adapters/mongo/planned_rest_days.rs`
- Modify: `src/adapters/mongo/mod.rs`, `src/main.rs` (ensure_indexes)

- [ ] **Step 1: Write integration-style unit test with test Mongo or in-memory fake first**

Prefer in-memory fake in domain tests; Mongo mapping test optional if repo follows existing `races` mongo shape.

- [ ] **Step 2: Implement `MongoPlannedRestDayRepository`**

Document struct + `map_document_to_domain` / `map_domain_to_document`.

Query for intersection:

```rust
doc! {
  "user_id": user_id,
  "start_date": { "$lte": range.newest },
  "end_date": { "$gte": range.oldest },
}
```

Indexes in `ensure_indexes()`:

```rust
// unique (user_id, planned_rest_day_id)
// (user_id, start_date, end_date)
```

- [ ] **Step 3: Wire repository in `main.rs`, call `ensure_indexes`**

- [ ] **Step 4: `CARGO_BUILD_JOBS=1 cargo test planned_rest_day -- --nocapture` — PASS**

---

### Task 4: REST CRUD

**Files:**
- Create: `src/adapters/rest/planned_rest_days/**`
- Modify: `src/adapters/rest/mod.rs`, `src/config/app_state.rs`

- [ ] **Step 1: Write failing REST test** (`tests/planned_rest_days_rest/crud.rs`)

```rust
#[tokio::test]
async fn create_list_get_update_delete_planned_rest_day_for_authenticated_user() {
    // POST body with 3-day range
    // GET list intersecting range returns 1 entry
    // PUT changes title
    // DELETE returns 204
    // GET by id returns 404
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `CARGO_BUILD_JOBS=1 cargo test create_list_get_update_delete_planned_rest_day --test planned_rest_days_rest -- --nocapture`

- [ ] **Step 3: Implement handlers** (copy races structure)

Routes:

```rust
.route("/api/planned-rest-days", get(list).post(create))
.route("/api/planned-rest-days/:planned_rest_day_id", get(get_one).put(update).delete(delete_one))
```

DTO validation at transport boundary: trim title/note, reject empty body fields that are optional.

- [ ] **Step 4: Run REST tests — PASS**

- [ ] **Step 5: `CARGO_BUILD_JOBS=1 cargo clippy --all-targets --all-features -- -D warnings`**

---

### Task 5: Calendar labels

**Files:**
- Create: `src/adapters/mongo/planned_rest_days_calendar.rs`, `src/domain/calendar_labels/composite.rs`
- Modify: `src/domain/calendar_labels/model.rs`, `src/adapters/rest/calendar/dto.rs`, `mapping.rs`, `src/main.rs`

- [ ] **Step 1: Extend `CalendarLabelPayload`**

```rust
PlannedRestDay(CalendarPlannedRestDayLabel {
    planned_rest_day_id: String,
    start_date: String,
    end_date: String,
    title: Option<String>,
    note: Option<String>,
}),
```

Update `kind()` → `"planned_rest_day"`.

- [ ] **Step 2: Implement `MongoPlannedRestDayCalendarLabelSource`**

For each intersecting `PlannedRestDay`, expand each date in range to a `CalendarLabel`.

- [ ] **Step 3: Implement `CompositeCalendarLabelSource`**

```rust
pub struct CompositeCalendarLabelSource<A, B> {
    primary: A,
    secondary: B,
}

// list_labels: join primary.list_labels().await? with secondary.list_labels().await?
```

Wire: `CompositeCalendarLabelSource::new(race_calendar_source, planned_rest_calendar_source)`.

- [ ] **Step 4: REST label DTO + mapping tests**

Extend `CalendarLabelDto` enum variant; update frontend Zod in Task 7.

- [ ] **Step 5: Integration test** (`tests/planned_rest_days_rest/labels.rs`)

Create 2-day range → `GET /api/calendar/labels?oldest=&newest=` → 2 dates each have `kind=planned_rest_day`.

---

### Task 6: Training context + LLM legend

**Files:**
- Modify: `src/domain/training_context/model.rs`, `service/mod.rs`, `packing/payloads/stable.rs`, `src/domain/llm/context_prelude.rs`

- [ ] **Step 1: Failing builder test**

```rust
#[tokio::test]
async fn builder_includes_future_planned_rest_days_in_stable_payload() {
    // seed in-memory planned rest repo with range inside horizon
    // build training context
    // assert context.planned_rest_days.len() == 1
    // assert rendered.stable_context contains "\"prd\":"
}
```

- [ ] **Step 2: Add `planned_rest_days` field to `TrainingContext`**

Inject `PlannedRestDayRepository` into `DefaultTrainingContextBuilder` (optional port with noop default for existing tests).

- [ ] **Step 3: Load intersecting `[today, today + STABLE_FUTURE_EVENT_DAYS]`**

When `meso_upcoming_end` is set, use `max(horizon_end, meso_upcoming_end)`.

- [ ] **Step 4: Pack `prd` in `StablePayload`**

```rust
#[derive(Serialize)]
struct CompactPlannedRestDay<'a> {
    id: &'a str,
    sd: &'a str,
    ed: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nt: Option<&'a str>,
}
```

- [ ] **Step 5: Update `PACKED_TRAINING_CONTEXT_LEGEND`**

Add: `prd=athlete-declared planned rest ranges (sd/ed inclusive); use as scheduling constraints; distinct from pd rest and av availability`.

- [ ] **Step 6: Run targeted tests**

Run: `CARGO_BUILD_JOBS=1 cargo test training_context -- --nocapture` (narrow filters if slow)

---

### Task 7: Frontend API + types

**Files:**
- Create: `frontend/src/features/planned-rest-days/types.ts`, `api/plannedRestDays.ts`, `api/plannedRestDays.test.ts`

- [ ] **Step 1: Zod schemas**

```typescript
export const plannedRestDaySchema = z.object({
  plannedRestDayId: z.string(),
  startDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  endDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  title: z.string().nullable(),
  note: z.string().nullable(),
  createdAtEpochSeconds: z.number().int(),
  updatedAtEpochSeconds: z.number().int(),
});

export const upsertPlannedRestDayRequestSchema = z.object({
  startDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  endDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  title: z.string().trim().max(120).nullable().optional(),
  note: z.string().trim().max(2000).nullable().optional(),
}).refine((v) => v.endDate >= v.startDate, { message: 'endBeforeStart' });
```

- [ ] **Step 2: API functions** — `listPlannedRestDays`, `createPlannedRestDay`, `updatePlannedRestDay`, `deletePlannedRestDay`

Use `httpClient` pattern from `frontend/src/features/races/api/races.ts`.

- [ ] **Step 3: Run tests**

Run: `bun run --cwd frontend test src/features/planned-rest-days/api/plannedRestDays.test.ts`
Expected: PASS

- [ ] **Step 4: Extend calendar label schema**

Add `planned_rest_day` variant to `calendarLabelSchema` in `frontend/src/features/calendar/types.ts`.

---

### Task 8: Frontend management page

**Files:**
- Create: `frontend/src/features/planned-rest-days/hooks/usePlannedRestDays.ts`, `components/PlannedRestDaysPageLayout.tsx`, `components/PlannedRestDayForm.tsx`, `components/PlannedRestDayCard.tsx`
- Create: `frontend/src/pages/PlannedRestDaysPage.tsx`
- Modify: `frontend/src/App.tsx`, `frontend/src/app/AuthenticatedLayout.tsx`, `frontend/src/i18n.ts`

- [ ] **Step 1: Hook** — load range `today-30d` .. `today+400d`, split upcoming/past

- [ ] **Step 2: Form with mode toggle**

```tsx
type DateMode = 'single' | 'range';
// single: one <input type="date"> copies to both start/end
// range: start + end inputs
```

- [ ] **Step 3: Page layout** — mirror races hero + list sections

- [ ] **Step 4: Routing + nav**

```tsx
// App.tsx
<Route element={<PlannedRestDaysPage apiBaseUrl={API_BASE_URL} />} path="/planned-rest-days" />

// AuthenticatedLayout.tsx — after Races
<NavItem to="/planned-rest-days" icon={BedDouble} label={t('nav.plannedRestDays')} />
```

- [ ] **Step 5: i18n keys** — `plannedRestDays.*`, `nav.plannedRestDays`, `appShell.pageTitles.plannedRestDays`

- [ ] **Step 6: Tests**

Run: `bun run --cwd frontend test src/features/planned-rest-days`

---

### Task 9: Calendar rendering

**Files:**
- Modify: `frontend/src/features/calendar/dayItems.ts`, `components/CalendarDayCell.tsx`, `CalendarMobileList.tsx`, `DayItemsModal.tsx`, `hooks/useCalendarData.ts` (cache bust export if needed)

- [ ] **Step 1: `dayItems.ts` — emit planned rest items from labels**

```typescript
| {
    kind: 'planned_rest_day';
    id: string;
    title: string;
    subtitle: string | null;
    dateKey: string;
    payload: PlannedRestDayLabelPayload;
  }
```

- [ ] **Step 2: `CalendarDayCell` visual**

When `plannedRestLabel` present and no higher-priority content:

- border: `border-violet-400/50`
- icon: `BedDouble`
- title: label title or `t('calendar.plannedRestDay')`
- subtitle: note or multi-day range hint

Do not set `isPlannedRestDay` from `event.restDay` — separate code path.

- [ ] **Step 3: Content test**

```typescript
it('renders user planned rest day label with violet styling', () => {
  const day = makeCalendarDay({
    labels: [{
      kind: 'planned_rest_day',
      title: 'Family holiday',
      subtitle: 'No cycling',
      payload: { plannedRestDayId: 'prd:1', startDate: '2026-07-01', endDate: '2026-07-07', title: 'Family holiday', note: 'No cycling' },
    }],
  });
  // assert violet border + title
});
```

- [ ] **Step 4: After planned-rest CRUD, bump calendar refresh**

From management page optional; calendar page already polls labels — document manual refresh or add `labelsCacheEpoch` bump via shared util if races doesn't already.

- [ ] **Step 5: Run calendar tests**

Run: `bun run --cwd frontend test src/features/calendar/components/CalendarDayCell.content.test.tsx`

---

### Task 10: Final verification

- [ ] **Architecture**

Run: `bun run verify:arch`
Expected: PASS (no domain → adapter imports)

- [ ] **Rust format + clippy**

```bash
CARGO_BUILD_JOBS=1 cargo fmt --all --check
CARGO_BUILD_JOBS=1 cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] **Targeted backend tests**

```bash
CARGO_BUILD_JOBS=1 cargo test --test planned_rest_days_rest -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test training_context -- --test-threads=1
```

- [ ] **Frontend**

```bash
bun run --cwd frontend test src/features/planned-rest-days src/features/calendar/components/CalendarDayCell.content.test.tsx
bun run --cwd frontend build
```

- [ ] **Graphify** (if code changed)

Run: `./scripts/rebuild_graphify.sh`

---

## Self-review checklist

| Spec requirement | Task |
| --- | --- |
| Mongo CRUD | 1–4 |
| Date range + single day | 1–2, 8 |
| Separate nav section | 8 |
| Calendar overlay | 5, 9 |
| LLM context | 6 |
| User scoping | 4 |
| Distinct from AI rest day | 9 (visual + no synthetic events) |
| i18n | 8 |

## Execution handoff

Plan complete at `docs/superpowers/plans/2026-06-10-planned-rest-days.md`.

**1. Subagent-Driven (recommended)** — fresh subagent per task, review between tasks

**2. Inline Execution** — execute in this session with checkpoints

Which approach?
