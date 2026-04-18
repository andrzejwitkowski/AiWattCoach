# AGENTS.md

This file is for coding agents working in `AiWattCoach`.

## Project Overview

- Backend: Rust, Axum, MongoDB, reqwest, tracing, OpenTelemetry.
- Frontend: Bun, Vite, React, TypeScript, Zod, Vitest, Tailwind.
- Deploy shape: one Rust server also serves the built SPA from `frontend/dist`.
- Architecture: hexagonal / ports-and-adapters style.

## Instruction Sources

- Before starting meaningful work, read the Obsidian handbook entry at `/Users/andrzej.witkowski/obsidian/andrzej.witkowski/opencode/OpenCode Start Here.md`.
- On Windows in this environment, check the Obsidian vault under `E:\obsidian\vault`.
- Then read the linked notes relevant to the task, especially architecture, Mongo reliability, HTTP adapter, and completion-checklist notes.
- Override any generic superpowers/worktree behavior: implement the feature in the folder where OpenCode was invoked. Do not create or switch to git worktrees for normal task execution in this repo.
- No repo-local Cursor rules were found in `.cursor/rules/`.
- No `.cursorrules` file was found.
- No Copilot instructions were found in `.github/copilot-instructions.md`.
- Follow this file plus the existing code patterns in the repo.

## Obsidian Priorities

- Core priorities from the Obsidian handbook:
  1. correct durable local state
  2. crash-safe recovery around external APIs
  3. clear hexagonal boundaries
  4. small, testable modules
  5. honest verification before calling work done
- Non-negotiable rule: persist local state before side effects.
- Do not claim completion without checking real command output.

## High-Level Architecture Rules

- Keep domain logic in `src/domain/**`.
- Keep infrastructure and external API details in `src/adapters/**`.
- Keep Axum handler code thin; delegate behavior to services/use cases.
- When a backend area grows large, split it into directory modules with `mod.rs` plus focused siblings by concern such as `dto.rs`, `handlers.rs`, `mapping.rs`, `error.rs`, `validation.rs`, or feature-specific files.
- Keep Mongo document shapes separate from domain models.
- Keep Intervals.icu DTOs inside `src/adapters/intervals_icu/**`.
- Keep LLM provider DTOs and SDK types inside dedicated adapter folders as well.
- Map external DTOs to internal models at adapter boundaries.
- Do not leak reqwest, Axum, or Mongo types into domain code.
- Prefer explicit repository and API ports over direct cross-layer calls.
- Preserve per-user scoping in REST endpoints and repositories.
- Persist local state before external side effects when a workflow requires both.
- Use durable checkpoints or operation records for retry-sensitive external flows.

## Search / Change Strategy

Before changing behavior, use this order:
1. relevant Obsidian note in `opencode/`
2. current production code path in the repo
3. existing tests for the same behavior
4. integration and recovery path if external APIs are involved
5. only then propose or implement the change

## Karpathy Guardrails for Agents

Adapted from `forrestchang/andrej-karpathy-skills` for OpenCode work in this repo.

- Think before changing:
  - State assumptions when they matter.
  - If multiple valid interpretations exist, surface them instead of silently picking one.
  - If a simpler approach works, prefer it and say so.
  - If something important is unclear, stop and ask instead of guessing.
- Simplicity first:
  - Write the minimum code that solves the requested problem.
  - Do not add speculative abstractions, flexibility, or configurability that were not requested.
  - Avoid handling impossible scenarios just to look complete.
  - If a solution feels overbuilt, reduce it before shipping it.
- Surgical changes:
  - Touch only what is needed for the request.
  - Do not refactor unrelated code, formatting, or comments while making a focused change.
  - Match existing local style unless the task explicitly includes a broader cleanup.
  - Remove only the imports, variables, or helpers that your change made unnecessary.
- Goal-driven verification:
  - Turn requests into verifiable outcomes before editing.
  - Prefer checks that prove the requested behavior, not generic confidence.
  - For multi-step work, keep a short plan with a concrete verification step for each part.
  - Do not call the work done until the relevant commands or checks were actually run and their output was read.

## Important Directories

- `src/domain/` - domain models, ports, services.
- `src/adapters/rest/` - Axum HTTP handlers and routing.
- `src/adapters/mongo/` - Mongo repositories and document mapping.
- `src/adapters/intervals_icu/` - Intervals.icu HTTP client and DTOs.
- `src/config/` - app state and settings.
- `frontend/src/` - React app code.
- `tests/` - backend integration and domain tests.
- `docs/plans/` - implementation plans and design notes.

## Setup Commands

- Install root tooling: `bun install`
- Install frontend deps only: `bun install --cwd frontend`
- Run backend locally: `cargo run`
- Run frontend dev server: `bun run --cwd frontend dev`
- Run Docker dev stack: `docker compose up --build`

## Build Commands

- Build backend: `cargo build`
- Build frontend: `bun run --cwd frontend build`
- Build everything through CI-like flow: `bun run verify:all`
- Build Docker image: `docker build -t aiwattcoach:ci .`

## Format Commands

- Format Rust: `cargo fmt --all`
- Check Rust formatting: `cargo fmt --all --check`
- Frontend formatting is not driven by a dedicated formatter script here; preserve existing style and rely on TypeScript/Vitest/build checks.

## Lint Commands

- Run Rust clippy exactly like CI: `cargo clippy --all-targets --all-features -- -D warnings`
- Run repo Rust verification shortcut: `bun run verify:rust`
- Run architecture boundary verification: `bun run verify:arch`

## Test Commands

- Run all backend tests: `cargo test`
- Run all frontend tests: `bun run --cwd frontend test`
- Run all checks in repo: `bun run test:all`

## Single-Test Commands

- Run one Rust integration test file: `cargo test --test intervals_rest`
- Run one Rust test by name: `cargo test list_events_returns_events_for_authenticated_user --test intervals_rest -- --nocapture`
- Run one Rust unit test by name: `cargo test normalize_email_trims_and_lowercases_values -- --nocapture`
- Run one frontend test file: `bun run --cwd frontend test src/features/intervals/api/intervals.test.ts`
- Run multiple frontend test files: `bun run --cwd frontend test src/features/a.test.ts src/features/b.test.ts`
- Vitest watch mode: `bun run --cwd frontend test:watch`

## CI / Hook Expectations

- CI runs:
  - `bun run verify:arch`
  - `cargo fmt -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - `bun run --cwd frontend test`
  - `bun run --cwd frontend build`
- Pre-commit hook runs `bun run verify:rust` and `bun run verify:arch` when staged Rust files exist.
- Pre-push hook runs `bun run verify:all`.
- Before finishing meaningful Rust work, run at least `bun run verify:arch`, `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and the relevant tests.

## Rust Style Guidelines

- Use Rust 2021 idioms and keep code clippy-clean with `-D warnings`.
- Use `rustfmt`; do not manually fight formatting.
- Aim for borrow-checker simplicity: prefer ownership and narrow borrows over fancy `Arc`, nested references, lifetime-heavy APIs, or other clever patterns unless they are clearly and concretely needed.
- Prefer small structs/enums with explicit field names over tuples.
- Derive traits explicitly and minimally (`Clone`, `Debug`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`) based on actual need.
- Prefer `Option<T>` for truly optional fields, not sentinel values.
- Prefer `String` in owned domain/config models; use `&str` in APIs where borrowing is simple.
- Use `impl std::fmt::Display` and `impl std::error::Error` for domain/config error enums.
- Return repository/API/service futures as the port traits expect; mirror existing `BoxFuture` patterns.
- Avoid needless lifetimes and needless clones; clippy is configured to catch several of these.
- Prefer explicit mapping helpers like `map_document_to_domain`, `map_domain_to_document`, `map_event_response`.
- Prefer `snake_case` for fields/functions/modules; use `PascalCase` for types and traits.
- Use descriptive enum variants like `CredentialsNotConfigured`, `Unauthenticated`, `NotFound`.

## Import Conventions

- `src/domain/**` must not import `crate::adapters::**`; this is enforced by `cargo_pup`.

- Group imports by std / external crates / crate-local modules.
- Use nested imports when they improve readability.
- Keep imports ordered as `rustfmt` wants.
- Alias only when necessary, usually for trait imports like `TracerProvider as _`.

## Error Handling Guidelines

- Do not `unwrap()` in production code unless failure is impossible and obvious.
- In tests, `unwrap()` and `expect()` are acceptable when they improve signal.
- Map infrastructure errors into domain or adapter-specific errors with explicit `map_err` closures.
- Convert HTTP/integration failures into domain-friendly enums before returning upstream.
- In REST handlers, translate domain errors into HTTP responses centrally where possible.

## Backend Architectural Conventions

- Domain services should orchestrate use cases, not know about HTTP or Mongo.
- REST DTOs may differ from domain models; keep JSON naming explicit with `#[serde(rename = ...)]`.
- REST handlers that accept variant payloads must validate mutual-exclusion and non-empty rules at the transport boundary before calling services or external APIs.
- When a handler decodes request bodies into memory (for example base64 file payloads), enforce both request body limits and decoded-size limits in the HTTP adapter.
- Mongo repositories should:
  - define internal `Document` structs
  - create indexes in `ensure_indexes()`
  - map documents explicitly
  - scope lookups by `user_id` when applicable
- Intervals adapter methods should:
  - authenticate with Basic Auth as implemented today
  - keep Intervals payload fields in DTO structs
  - normalize external field names into internal names before returning domain models

## Frontend Style Guidelines

- Use TypeScript strictly and validate API payloads with Zod at the boundary.
- Keep schemas in feature `types.ts` files and parse responses in API helpers.
- Use `camelCase` in TypeScript and React code.
- Keep API functions small and focused: validate input, call HTTP helper, parse output.
- Reuse shared HTTP utilities from `frontend/src/lib/httpClient.ts`.
- Prefer feature-local tests next to or near the feature API/components.

## Testing Conventions

- Write tests before backend behavior changes whenever practical.
- Add or update tests with behavior changes.
- Prefer focused integration tests in `tests/*.rs` for HTTP and adapter behavior.
- When an integration test target or shared test helper grows large, split it into a directory-based suite such as `tests/<suite>/main.rs` with focused files for support, fixtures, fakes, observability, and behavior groups.
- Use fakes/test doubles for domain-service tests.
- Verify user scoping in REST tests whenever endpoints are user-owned.
- For frontend API tests, mock `fetch` and validate parsed output shapes.
- For retry-sensitive workflows, test idempotency and recovery behavior, not just happy paths.

## Logging / Telemetry Notes

- This repo uses `tracing`, `tracing-subscriber`, and OpenTelemetry.
- Preserve trace propagation code and structured logging behavior.
- Do not remove telemetry wiring just to satisfy tests; fix imports/config instead.

## Logging Walkthrough

- Read `docs/logging.md` before adding new endpoint or external client logging.
- For REST endpoints:
  - keep logging in `src/adapters/rest/logging/**`
  - add `RequestLogLayer` only when route-level body logging is needed
  - configure the route with `with_log_config(EndpointLogConfig::...)`
  - add `DefaultBodyLimit::max(...)` before `RequestLogLayer` when the logger can buffer request bodies
  - keep preview limits small with `with_max_body_bytes(...)`
- For outbound clients:
  - keep logging in the adapter client module, not handlers or domain services
  - preserve trace propagation before sending the request
  - default to `execute_and_log_no_body(...)` or an adapter helper like `execute_and_log_with_trace_no_body(...)`
  - use full body preview logging only for narrowly scoped safe payloads
  - prefer payload summaries like `payload bytes=... hash=...` for malformed or unsafe upstream bodies
- For both endpoints and clients:
  - redact secrets before preview logging
  - do not log raw large binary payloads
  - add or update targeted observability tests when logging behavior changes

## External API / Workflow Safety

- Do not call Intervals or future LLM providers directly from REST handlers.
- Do not let external DTOs leak into repositories, domain services, or use-case interfaces.
- Translate external/provider errors into internal error categories at the adapter boundary.
- Do not assume retries are harmless; add dedupe or durable workflow state first.
- Recovery-critical workflows must not depend on in-memory-only flags.

## Environment Notes

- Backend reads `.env` automatically during local startup.
- Frontend may use `frontend/.env` for API origin overrides.
- Mongo readiness matters; `/ready` checks database availability.

## When Editing

- Match existing file style before introducing new patterns.
- Prefer minimal diffs.
- Keep files under 500 lines of code when practical; refactor early into smaller logical modules/components instead of letting one file grow into spaghetti code.
- Do not keep catch-all support files after a split; move shared helpers into narrowly scoped modules like `app.rs`, `identity.rs`, `settings.rs`, `intervals.rs`, or similar concern-based names.
- Keep the borrow checker boring; redesign API shapes before introducing extra `clone()`, `Arc`, shared wrappers, or explicit lifetimes.
- Do not rename public API fields casually.
- Update tests when adding endpoints, DTO fields, or repository behavior.
- If you add a new backend capability, check whether frontend schemas and API helpers must also change.

## Review Reply Rule

- When a PR has Copilot, CodeRabbit, or human review comments, read every actionable comment and evaluate it against the current code before replying.
- If a review comment is correct, implement the fix, run the relevant verification, and then reply on that exact PR thread with a concise note describing what changed.
- If a review comment is not correct or is intentionally not adopted, still reply on that exact PR thread with a concise explanation grounded in the current code and behavior.
- Do not leave review comments unanswered when the task includes handling review feedback.

## Global Review Loop

- After implementing any non-trivial feature or behavior change, run a 4-iteration review loop before calling the work done unless the user explicitly narrows verification.
- In each iteration, do 3 review phases in this order: strict reviewer, very strict reviewer, nitpicker.
- After each iteration, convert confirmed findings into a concrete fix plan, implement the fixes, and rerun the most relevant verification.
- Continue through all 4 iterations unless you are blocked by a real ambiguity, missing secret, or destructive decision that requires user input.
- Treat review findings as untrusted until checked against the current code and tests; do not cargo-cult speculative comments into the codebase.

## Done Checklist For Agents

Before saying the task is done, verify all of the following:

- I can state the task in one sentence.
- I changed only what was needed.
- Domain code still stays independent from Axum, Mongo, and provider SDKs.
- Handlers remain thin and only map transport concerns.
- Recovery-critical local state is persisted before external side effects.
- Retry-sensitive flows remain idempotent or have durable checkpoints.
- I added or updated relevant tests.
- I ran the most relevant targeted tests.
- I ran the final verification command.
- I read the command output, not just the exit code.

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- After modifying code files in this session, run `./scripts/rebuild_graphify.sh` to keep the graph current
