# Planned Rest Days Design

**Goal:** Let athletes record future rest days (single days or date ranges such as holidays), manage them with full CRUD in a dedicated UI section, show them on the main calendar, and feed them into AI coach packed training context.

**Architecture:** New user-scoped domain `planned_rest_days` persisted in Mongo, exposed via REST, merged into `/api/calendar/labels` for calendar rendering, and loaded by `DefaultTrainingContextBuilder` into stable packed context. No Intervals sync in v1 — this is local athlete intent, distinct from AI-generated training-plan rest days and Intervals-imported special days.

**Tech stack:** Rust (domain / Mongo / Axum), React + TypeScript + Zod + Vitest, existing calendar labels + training context packing pipeline.

---

## Problem

Athletes need to mark deliberate future rest blocks (recovery weeks, vacations, travel) that:

- are **not** the same as weekly availability in settings (recurring capacity),
- are **not** the same as AI-prescribed `rest_day` entries inside the 14-day training-plan projection,
- are **not** the same as Intervals-imported `special_days` (illness, travel notes from external calendar).

Today there is no durable place to store this intent, no management UI, no calendar marker, and no LLM context field the coach can trust.

## Scope

### In scope

- Mongo-backed `planned_rest_days` collection with user scoping
- REST CRUD: list (date range), get, create, update, delete
- Dedicated nav section + page at `/planned-rest-days`
- Create/edit form supporting:
  - single day (`start_date == end_date`)
  - inclusive date range (e.g. one-week holiday)
  - optional title (e.g. "Ski trip") and optional note
- Main calendar (`/calendar`) shows a distinct planned-rest marker on affected days via calendar labels
- Day modal / mobile list includes planned-rest items
- Stable packed training context includes future planned rest days for all coach surfaces that already use `TrainingContextBuilder` (workout coach, calendar coach, training-plan generation, meso cycle)
- i18n strings (EN at minimum; follow existing `i18n.ts` patterns)
- Focused backend integration tests + frontend API/component tests

### Out of scope (v1)

- Pushing rest days to Intervals.icu or Wahoo
- Automatic training-plan rewrite when a planned rest day is added
- Recurring rest patterns (e.g. "every Monday")
- Blocking workout creation on rest days
- Meso-cycle calendar page markers (can follow in a small follow-up once main calendar works)
- Admin prompt-preview decode legend updates (nice-to-have; not blocking)

## Non-goals / distinctions

| Concept | Source | v1 behavior |
| --- | --- | --- |
| Weekly availability | Settings | Unchanged; capacity template only |
| AI rest day | Training-plan projection (`restDay` on predicted events) | Unchanged; coach-generated |
| Intervals special day | External import | Unchanged; read-only import |
| **Planned rest day** | User CRUD in app | **New**; athlete-declared future rest |

Visual distinction on calendar:

- AI / projected rest day: existing orange-red border (`restDay` on calendar event)
- User planned rest day: new violet/indigo accent via calendar label (no fake Intervals event)

## Data model

### Collection: `planned_rest_days`

| Field | Type | Notes |
| --- | --- | --- |
| `planned_rest_day_id` | string | Stable id, e.g. `prd:{user_id}:{uuid}` |
| `user_id` | string | Required on every query |
| `start_date` | string | `YYYY-MM-DD`, inclusive |
| `end_date` | string | `YYYY-MM-DD`, inclusive, `>= start_date` |
| `title` | string? | Short label; default UI title when null |
| `note` | string? | Optional longer reason |
| `created_at_epoch_seconds` | i64 | Set on create |
| `updated_at_epoch_seconds` | i64 | Set on create/update |

Indexes:

- unique `(user_id, planned_rest_day_id)`
- `(user_id, start_date, end_date)` for range queries

### Domain type: `PlannedRestDay`

```rust
pub struct PlannedRestDay {
    pub planned_rest_day_id: String,
    pub user_id: String,
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}
```

Validation rules:

- Dates must parse as `YYYY-MM-DD`
- `end_date >= start_date`
- Range length max **366 days** (reject longer spans at REST boundary)
- `title` max 120 chars, `note` max 2000 chars
- Create/update allowed only when `end_date >= user_today_utc` (past-only ranges rejected on write; historical rows remain readable after they pass)

Overlap policy (v1): **allow overlaps**. Multiple entries may cover the same day; calendar shows each label; LLM context lists each range. Simpler than merge logic and matches "I marked vacation + recovery" edge cases. Document in API validation messages.

## API

Base path: `/api/planned-rest-days`

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/api/planned-rest-days?oldest=&newest=` | List entries intersecting `[oldest, newest]` |
| GET | `/api/planned-rest-days/{id}` | Get one; 404 if missing or wrong user |
| POST | `/api/planned-rest-days` | Create |
| PUT | `/api/planned-rest-days/{id}` | Full update of mutable fields |
| DELETE | `/api/planned-rest-days/{id}` | Delete |

Request body (create/update):

```json
{
  "startDate": "2026-07-01",
  "endDate": "2026-07-07",
  "title": "Family holiday",
  "note": "No bike; hiking only"
}
```

Response DTO mirrors domain with camelCase JSON.

Auth: same `resolve_user_id` pattern as races; all operations user-scoped.

## Calendar integration

### Label payload

Extend `CalendarLabelPayload` with:

```rust
PlannedRestDay(CalendarPlannedRestDayLabel {
    planned_rest_day_id: String,
    start_date: String,
    end_date: String,
    title: Option<String>,
    note: Option<String>,
})
```

`kind` string: `"planned_rest_day"`.

For each day `D` in `[start_date, end_date]`, emit one label:

- `label_key`: `planned_rest_day:{planned_rest_day_id}`
- `date`: `D`
- `title`: title or i18n default `"Planned rest"`
- `subtitle`: note or formatted range when multi-day

### Label source wiring

Add `MongoPlannedRestDayCalendarLabelSource` and a small `CompositeCalendarLabelSource` in `domain/calendar_labels` that concatenates race labels (existing `MongoCalendarEntryViewCalendarSource`) with planned-rest labels. Wire composite in `main.rs` instead of race-only source.

No change to `/api/calendar/events` — planned rest days are overlays, not synthetic Intervals events.

### Frontend calendar

- Extend `calendarLabelSchema` Zod union with `planned_rest_day` kind
- `CalendarDayCell`: when label present, show `BedDouble` icon + violet border/background accent (distinct from AI rest-day orange)
- `dayItems.ts`: add `planned_rest_day` item kind; non-interactive (like generic events) unless we later add edit shortcut
- `DayItemsModal` / mobile list: list planned rest entries for the day
- Invalidate labels cache on planned-rest CRUD (same pattern as races refresh)

## Training context / LLM

Add to `TrainingContext`:

```rust
pub planned_rest_days: Vec<PlannedRestDayContext>,
```

```rust
pub struct PlannedRestDayContext {
    pub planned_rest_day_id: String,
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
}
```

Load in `DefaultTrainingContextBuilder::build_impl`:

- Query `planned_rest_days` intersecting `[today, today + STABLE_FUTURE_EVENT_DAYS]` (same horizon as `fe` future events — currently 90 days; extend query if builder uses longer meso windows)
- For meso builds with extended `upcoming_end`, widen query to cover that end date

Stable packed payload (`StablePayload`):

- New field `prd: Vec<CompactPlannedRestDay>` with `{ id, sd, ed, n?, nt? }`

Update `PACKED_TRAINING_CONTEXT_LEGEND`:

- Document `prd=athlete-declared planned rest day ranges (future holidays/recovery); sd/ed inclusive; distinct from AI rest_day in pd and from weekly availability av`

Coach behavior expectation (prompt-level, no new tools v1):

- Treat `prd` as hard scheduling constraints the athlete chose
- Do not recommend hard sessions on those dates
- When discussing upcoming weeks, mention declared rest blocks by title when present

## Frontend page: Planned Rest Days

Route: `/planned-rest-days`

Nav: new item in `AuthenticatedLayout` sidebar (icon: `BedDouble` or `Palmtree`), placed after Races. Not added to mobile bottom nav in v1 (same as Races/Meso — desktop-first management page).

Layout (mirror `RacesPageLayout`):

- Hero with metrics: upcoming rest days count, next rest block, total days off in next 90 days
- **Add rest days** button opens modal/sheet form
- Sections: **Upcoming** / **Past**
- Card per entry: date range (formatted), title, note preview, edit + delete actions

Form (`PlannedRestDayForm`):

- Toggle: **Single day** | **Date range**
- Single day: one date picker sets both start and end
- Range: start + end pickers with validation (`end >= start`)
- Title (optional), Note (optional textarea)
- Delete confirmation on edit

Hook: `usePlannedRestDays({ apiBaseUrl })` — list, create, update, delete, refresh.

API module: `frontend/src/features/planned-rest-days/api/plannedRestDays.ts` with Zod schemas.

## Error handling

| Case | HTTP |
| --- | --- |
| Invalid dates / range | 400 |
| Range too long | 400 |
| End in past on create | 400 |
| Not found | 404 |
| Unauthenticated | 401 |
| Mongo failure | 500 mapped to domain error |

## Testing

### Backend

- Domain service unit tests: validation, date intersection listing
- REST integration tests (`tests/planned_rest_days_rest/`): auth scoping, CRUD happy path, range query, validation failures
- Calendar labels integration: label appears for each day in range
- Training context builder test: `prd` populated for future range

### Frontend

- API parse tests with mocked `fetch`
- Form validation tests (single vs range)
- `CalendarDayCell` test: renders planned rest label styling
- Page layout smoke test

## Migration / rollout

- New collection only; no backfill
- Deploy backend before frontend (new endpoints + labels are backward compatible; old frontend ignores unknown label kinds)
- Frontend can ship in same release

## Open decisions (defaults chosen)

| Question | v1 default |
| --- | --- |
| Sync to Intervals? | No |
| Overlapping ranges? | Allowed |
| Past dates on create? | Reject if entire range ends before today |
| Edit past entries? | Allow title/note edit; disallow changing dates if range fully in past |
| Meso calendar markers? | Follow-up task |

## Success criteria

- Athlete can create a 7-day holiday range and see all 7 days marked on `/calendar`
- Athlete can edit title and delete the range
- Calendar coach packed context includes the range in `prd`
- No domain imports from adapters; handlers stay thin
