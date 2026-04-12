# Pest Parser PoC Workout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Pest-based shadow parser for Intervals.icu `workout_doc`, persist both successful and failed parse attempts into Mongo collection `pest_parser_poc_workout`, and trigger that persistence on every inbound Intervals read and every outbound sync/push to Intervals without changing the current application behavior.

**Architecture:** Keep the current public `parse_workout_doc` behavior intact. Introduce a new internal Pest parser with a local AST and error type inside `src/domain/intervals/workout/`. Add a dedicated Mongo adapter for PoC persistence, then wrap the live Intervals API adapter so inbound `list_events`/`get_event` and outbound `create_event`/`update_event` perform observational parse-and-store side effects while never blocking the main Intervals flow.

**Tech Stack:** Rust 2021, Pest, MongoDB, tracing, existing Intervals adapter and domain models.

---

## File Structure

**Create:**
- `src/domain/intervals/workout/pest_parser/mod.rs`
- `src/domain/intervals/workout/pest_parser/workout.pest`
- `src/domain/intervals/workout/pest_parser/ast.rs`
- `src/domain/intervals/workout/pest_parser/error.rs`
- `src/domain/intervals/workout/pest_parser/parser.rs`
- `src/domain/intervals/pest_parser_poc.rs`
- `src/adapters/mongo/pest_parser_poc_workouts.rs`
- `tests/intervals_pest_parser_poc.rs`

**Modify:**
- `Cargo.toml`
- `src/domain/intervals/workout.rs`
- `src/domain/intervals/mod.rs`
- `src/domain/intervals/ports.rs`
- `src/adapters/mongo/mod.rs`
- `src/main.rs`
- `src/adapters/intervals_icu/client/api.rs`

## Execution Notes

- Keep the current `parse_workout_doc(...)` signature unchanged.
- Persistence happens only at the Intervals adapter boundary.
- Persist both parsed and failed observations.
- Observation failures must not break inbound or outbound Intervals flows.
- Follow TDD: write failing tests first, verify the failure, then implement the minimum code to pass.

## Task Groups

### 1. Parser scaffolding and dependency wiring
- Add `pest` and `pest_derive` dependencies.
- Add the `pest_parser` workout submodule.
- Add parser smoke tests that fail first.
- Introduce the initial AST and error types.

### 2. Grammar and AST parsing
- Implement the `.pest` grammar for the approved PoC subset.
- Add tests for simple FTP, repeat blocks, pace, HR/LTHR, ramp, cadence, and text metadata.
- Ensure malformed input returns a structured error and never panics.

### 3. Mapping into existing parsed workout model
- Map the AST into `ParsedWorkoutDoc` for comparison and persistence.
- Preserve current summary semantics and percent-zone projection where applicable.
- Keep unsupported richer syntax visible in normalized output and PoC payload.

### 4. PoC domain model and repository port
- Add record types for parsed and failed observations.
- Add a no-op repository implementation.
- Keep repository behavior behind a domain port.

### 5. Mongo adapter
- Add `MongoPestParserPocWorkoutRepository` for collection `pest_parser_poc_workout`.
- Create indexes for lookup by user/time and by status/time.
- Persist both parsed and failed records.

### 6. Safe observation helper
- Add a pure helper that observes a workout string, runs the Pest parser, builds a PoC record, and keeps a legacy projection from the current parser.
- No panics, no main-flow breakage.

### 7. Inbound Intervals observation
- Observe and persist on `list_events(...)`.
- Include `get_event(...)` as the single-event inbound path if the wiring stays small and testable.
- Return the same `Event` values as before.

### 8. Outbound Intervals observation
- Observe and persist before `create_event(...)` and `update_event(...)` HTTP calls.
- Do not block outbound sync if parser or Mongo persistence fails.

### 9. Verification
- Run parser-specific tests.
- Run Mongo adapter tests.
- Run relevant Intervals and sync-related tests.
- Run `cargo fmt --all --check`.
- Run `cargo clippy --all-targets --all-features -- -D warnings`.

## Final Verification Commands

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --test intervals_workout_analysis -- --nocapture`
- `cargo test --test intervals_pest_parser_poc -- --nocapture`
- `cargo test --test intervals_rest -- --nocapture`

## Success Criteria

- New Pest parser exists and is tested.
- `pest_parser_poc_workout` records are stored for inbound and outbound Intervals operations.
- Both success and failure cases are persisted.
- No syntax error can panic the process.
- Existing public behavior remains stable.
- Relevant tests, formatting, and clippy pass.
