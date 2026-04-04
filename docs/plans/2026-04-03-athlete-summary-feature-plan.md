# Athlete Summary Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a persisted athlete summary that can be created from settings, refreshed weekly, displayed read-only in settings, and injected into cached workout-summary coach prompts.

**Architecture:** Introduce a new `athlete_summary` domain/repository/service backed by Mongo with a Monday freshness rule. Expose dedicated REST endpoints for fetching and generating the summary, render it in a read-only settings card, and ensure workout-summary chat generates it when missing/stale before continuing with coach reply generation.

**Tech Stack:** Rust, Axum, MongoDB, React, TypeScript, Zod, Vitest, cargo test

---

### Task 1: Athlete summary domain and repository

**Files:**
- Create: `src/domain/athlete_summary/model.rs`
- Create: `src/domain/athlete_summary/ports.rs`
- Create: `src/domain/athlete_summary/service.rs`
- Create: `src/domain/athlete_summary/mod.rs`
- Create: `src/adapters/mongo/athlete_summary.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing tests**
- Add service tests for missing, stale, fresh, and forced refresh behavior.
- Add Mongo repository tests if existing patterns support it; otherwise cover via REST/service integration tests.

**Step 2: Run tests to verify failure**
Run focused cargo tests for new athlete-summary service tests.

**Step 3: Write minimal implementation**
- Add model, error, repository port, and service.
- Add Mongo repository with unique `user_id` index.
- Wire service/repository into app startup.

**Step 4: Re-run tests**
Run the same focused tests and confirm pass.

### Task 2: REST API for fetch and generate

**Files:**
- Create: `src/adapters/rest/athlete_summary/dto.rs`
- Create: `src/adapters/rest/athlete_summary/error.rs`
- Create: `src/adapters/rest/athlete_summary/handlers.rs`
- Create: `src/adapters/rest/athlete_summary/mapping.rs`
- Create: `src/adapters/rest/athlete_summary/mod.rs`
- Modify: `src/adapters/rest/mod.rs`
- Test: `tests/settings_rest/*` or new `tests/athlete_summary_rest/*`

**Step 1: Write failing REST tests**
- GET returns empty/no-summary state
- POST generates and returns saved summary
- auth and user scoping verified

**Step 2: Run tests to verify failure**
Run focused REST tests.

**Step 3: Write minimal implementation**
- Add handlers and route registration.
- Map Monday freshness state into response metadata.

**Step 4: Re-run tests**
Run focused REST tests and confirm pass.

### Task 3: Settings UI card with read-only summary textbox

**Files:**
- Create: `frontend/src/features/settings/api/athleteSummary.ts`
- Create: `frontend/src/features/settings/components/AthleteSummaryCard.tsx`
- Create: `frontend/src/features/settings/components/AthleteSummaryCard.test.tsx`
- Modify: `frontend/src/features/settings/types.ts`
- Modify: `frontend/src/pages/SettingsPage.tsx`

**Step 1: Write failing frontend tests**
- button hidden when prerequisites missing
- create/refresh button shown when AI + Intervals are configured
- read-only summary textbox shows fetched summary
- generate button updates UI state

**Step 2: Run tests to verify failure**
Run focused Vitest file.

**Step 3: Write minimal implementation**
- Add API client, schema, card, and page composition.
- Keep textbox non-editable.

**Step 4: Re-run tests**
Run focused Vitest file and confirm pass.

### Task 4: Inject athlete summary into cached coach prompt

**Files:**
- Modify: `src/adapters/llm/workout_summary_coach.rs`
- Modify: any app wiring needed to provide athlete-summary service to coach path
- Test: `tests/llm_adapters.rs`

**Step 1: Write failing test**
- assert stable request context includes stored athlete summary text

**Step 2: Run test to verify failure**
Run focused LLM adapter test.

**Step 3: Write minimal implementation**
- load or ensure athlete summary before building stable prompt
- include summary in stable cached context

**Step 4: Re-run test**
Run focused test and confirm pass.

### Task 5: Websocket system message for summary generation

**Files:**
- Modify: `src/adapters/rest/workout_summary/dto.rs`
- Modify: `src/adapters/rest/workout_summary/ws.rs`
- Modify: `frontend/src/features/coach/types.ts`
- Modify: `frontend/src/features/coach/hooks/useCoachChat.ts`
- Modify: chat UI tests if needed
- Test: `tests/llm_rest/workout_summary_flow.rs`

**Step 1: Write failing websocket test**
- when athlete summary is missing or stale, websocket sends system message before coach reply

**Step 2: Run test to verify failure**
Run focused websocket flow test.

**Step 3: Write minimal implementation**
- add `system_message` websocket type
- send `First the summary is being generated - wait a moment`
- frontend renders it inline

**Step 4: Re-run test**
Run focused websocket flow test and confirm pass.

### Task 6: Final verification

**Files:**
- No new files

**Step 1: Run backend targeted suites**
Run REST, LLM adapter, and websocket tests covering athlete summary.

**Step 2: Run frontend targeted tests**
Run `AthleteSummaryCard` and coach chat tests.

**Step 3: Run formatting and linting**
Run `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and relevant frontend tests.
