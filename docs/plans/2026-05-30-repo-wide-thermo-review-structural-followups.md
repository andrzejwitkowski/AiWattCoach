# Repo-Wide Thermo Review Structural Follow-ups

**Goal:** Turn the 2026-05-30 repo-wide thermo review into one tracked cleanup issue focused on the highest-confidence structural hotspots, without changing user-visible behavior unless a refactor exposes a real bug.

**Scope:**
- split confirmed god files and god modules along existing concern boundaries
- reduce AI Coach summary state split-brain and establish one clear summary owner for the page
- keep current domain boundaries explicit instead of letting orchestration, storage, and prompt assembly continue to accrete in single files

**Non-goals:**
- do not redesign product behavior, endpoint contracts, or persistence semantics just to make files smaller
- do not mix this cleanup with unrelated performance work, scheduler redesign, or provider behavior changes
- do not refactor lower-confidence medium hotspots until the high-severity items are under control

## Confirmed Findings

### High: `UserSettingsService` is a settings-domain god service

**Files:**
- `src/domain/settings/service.rs:91-103`
- `src/domain/settings/service.rs:201-412`
- `src/domain/settings/service.rs:422-592`
- `src/domain/settings/service.rs:594+`

**Why this matters:**
- one service owns settings persistence, LLM cache invalidation, Intervals poll-state lifecycle, FTP history seeding, training-load recompute, and a large block of test doubles/tests in the same file
- small settings changes now require re-reading unrelated sync and training-load behavior

**Refactor direction:**
- keep one thin settings orchestration service
- move side effects into focused helpers or collaborators by concern
- move tests and test doubles out of the production file

### High: `training_context` service remains a central god node

**Files:**
- `src/domain/training_context/service/mod.rs:81-241`
- `src/domain/training_context/service/mod.rs:365-607`
- `src/domain/training_context/service/mod.rs:623-868`
- `src/domain/training_context/service/mod.rs:871+`

**Why this matters:**
- the module owns repository ports, builder wiring, focus-date resolution, source loading, dedupe behavior, aggregate mapping, and prompt-context assembly in one place
- `build_impl(...)` and `resolve_focus_date(...)` keep absorbing new context rules, making the module the default dumping ground for future behavior

**Refactor direction:**
- split focus resolution, source loading, mapping to prompt models, and top-level orchestration into separate siblings under the existing directory module

### High: Mongo settings adapter is a storage god file with repeated schema logic

**Files:**
- `src/adapters/mongo/settings.rs:25-155`
- `src/adapters/mongo/settings.rs:265-398`
- `src/adapters/mongo/settings.rs:443-671`
- `src/adapters/mongo/settings.rs:673-922`

**Why this matters:**
- one file mixes document definitions, bootstrap queries, Wahoo backfill helpers, availability repair, full document mapping, and repository methods
- schema and timestamp-mirroring rules are duplicated between full-document mapping and multiple handwritten `$set` update paths

**Refactor direction:**
- split the adapter into focused modules such as `documents`, `mapping`, `bootstrap`, and `repository`
- centralize repeated timestamp/subdocument mapping logic instead of re-encoding fields in each partial update

### High: external import flow is still a domain god module

**Files:**
- `src/domain/external_sync/import/mod.rs:143-176`
- `src/domain/external_sync/import/mod.rs:309-575`
- `src/domain/external_sync/import/mod.rs:682-880`

**Why this matters:**
- one service handles planned workouts, completed workouts, races, special days, sync metadata persistence, canonical matching, link resolution, and calendar refresh triggering
- completed-workout import is especially overloaded and hard to change safely

**Refactor direction:**
- split by import type first
- extract completed-workout import flow into a dedicated module with focused helpers for canonical resolution, planned-workout link resolution, and sync metadata persistence

### High: `useCoachChat` still has split-brain summary and transcript state

**Files:**
- `frontend/src/features/coach/hooks/useCoachChat.ts:137-193`
- `frontend/src/features/coach/hooks/useCoachChat.ts:333-372`
- `frontend/src/features/coach/hooks/useCoachChat.ts:426-499`
- `frontend/src/features/coach/hooks/useCoachChat.ts:501-674`

**Why this matters:**
- `summary`, `messages`, `draftRpe`, and `summaryRef` overlap semantically, but different ingress paths mutate different subsets
- websocket tool/system/save events and REST save/reopen/send flows can still drift because there is no single canonical chat read model

**Refactor direction:**
- collapse onto one canonical local chat model, or make the split explicit as persisted summary state plus ephemeral UI-only messages

### High: Coach page summary ownership is spread across three stores

**Files:**
- `frontend/src/features/coach/components/CoachPageLayout.tsx:45-53`
- `frontend/src/features/coach/components/CoachPageLayout.tsx:80-85`
- `frontend/src/features/coach/components/CoachPageLayout.tsx:98-109`
- `frontend/src/features/coach/components/CoachPageLayout.tsx:193-199`
- `frontend/src/features/coach/hooks/useWorkoutList.ts:315-316`
- `frontend/src/features/coach/hooks/useWorkoutList.ts:441-485`

**Why this matters:**
- summary state is simultaneously owned by `useCoachChat`, `useCoachSessionCache`, and the workout-list sidebar cache
- page-level glue code keeps synchronizing the same summary across multiple stores after load, save, reopen, and cache hydration

**Refactor direction:**
- choose one summary owner for the page/session
- have chat and sidebar derive from that source instead of maintaining competing caches

### Medium: `llm_tools` mixes runtime loop, registry, and prompt policy

**Files:**
- `src/domain/llm_tools/mod.rs:189-427`
- `src/domain/llm_tools/mod.rs:429-515`
- `src/domain/llm_tools/mod.rs:528-674`

**Why this matters:**
- one module owns tool-loop runtime, checkpoint state, registry creation, scope filtering, prompt guidance, and name-based preview lookup
- tool metadata is not declared once, so runtime and prompt-policy logic can drift

**Refactor direction:**
- split into `loop`, `registry`, and `guidance` modules after the higher-severity cleanup items land

## Tasks

### Task 1: Split `UserSettingsService` by concern

**Files:**
- Replace: `src/domain/settings/service.rs`
- Create: `src/domain/settings/service/mod.rs`
- Create: focused siblings under `src/domain/settings/service/`

**Work:**
- keep the public service entrypoint stable
- move update side effects into focused helpers by concern
- move test doubles and tests out of the production file

**Done when:**
- the root service file reads as orchestration rather than a mixed settings/sync/training-load/test file

### Task 2: Split `training_context` service into focused builder phases

**Files:**
- Modify: `src/domain/training_context/service/mod.rs`
- Create: focused siblings under `src/domain/training_context/service/`

**Work:**
- extract focus-date resolution
- extract source loading and lookup helpers
- extract mapping from domain aggregates to prompt-context models
- keep `TrainingContextBuilder` as the stable entrypoint

**Done when:**
- `build_impl(...)` is no longer the place that knows every repository, every identity rule, and every prompt-shape transformation

### Task 3: Split Mongo settings adapter and de-duplicate mapping rules

**Files:**
- Replace: `src/adapters/mongo/settings.rs`
- Create: `src/adapters/mongo/settings/mod.rs`
- Create: focused siblings under `src/adapters/mongo/settings/`

**Work:**
- separate document types, mapping helpers, bootstrap/backfill queries, and repository methods
- reduce repeated timestamp/subdocument field encoding across update paths

**Done when:**
- document mapping and partial update logic no longer encode the same schema rules in several places

### Task 4: Split external import flow by import type

**Files:**
- Replace: `src/domain/external_sync/import/mod.rs`
- Create: focused siblings under `src/domain/external_sync/import/`

**Work:**
- keep the public `ExternalImportService` entrypoint stable
- move completed-workout import logic into a dedicated module
- keep sync metadata persistence and calendar refresh wiring explicit rather than scattered through one file

**Done when:**
- completed-workout import can evolve without re-auditing race/special-day/planned-workout flows in the same giant module

### Task 5: Give AI Coach one clear summary source of truth

**Files:**
- Modify: `frontend/src/features/coach/hooks/useCoachChat.ts`
- Modify: `frontend/src/features/coach/components/CoachPageLayout.tsx`
- Modify: `frontend/src/features/coach/hooks/useWorkoutList.ts`
- Modify tests under `frontend/src/features/coach/**`

**Work:**
- remove the remaining split-brain between `summary`, `messages`, and page/session/sidebar caches
- pick one page-level summary owner
- make chat and sidebar updates flow through that owner instead of hand-synchronizing several stores

**Done when:**
- chat transcript freshness and summary freshness are driven by one clear model

### Task 6: Optional `llm_tools` cleanup after the high-severity items

**Files:**
- Replace: `src/domain/llm_tools/mod.rs`
- Create: focused siblings under `src/domain/llm_tools/`

**Work:**
- split loop runtime, registry metadata, and prompt guidance generation
- keep tool metadata declared once where possible

**Done when:**
- new tools do not require editing loosely coupled registry/runtime/guidance logic in the same file

## Verification When Implementing

Run the most relevant focused tests for each slice as it lands, then at minimum run:

```bash
bun run verify:arch
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
bun run --cwd frontend test
```

If code files change during implementation, also run:

```bash
./scripts/rebuild_graphify.sh
```

## Exit Criteria

- the confirmed high-severity hotspots are no longer single-file review bottlenecks
- domain code still does not leak Axum, Mongo, or provider DTO concerns across boundaries
- AI Coach summary state has one clear owner and no ad hoc cache-sync triangle
- cleanup lands as minimal structural refactors, not a speculative architecture rewrite
