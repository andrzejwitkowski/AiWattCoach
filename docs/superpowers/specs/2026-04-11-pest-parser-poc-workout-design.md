# Pest Parser PoC Workout Design

**Goal:** Add a Pest-based shadow parser for Intervals.icu `workout_doc`, persist both successful and failed parse attempts to Mongo collection `pest_parser_poc_workout`, and trigger persistence on every inbound Intervals read and every outbound push/sync to Intervals without breaking the current application flow.

## Scope

Primary target:
- `workout_doc` parsing only

Out of scope for this PoC:
- replacing the current public `parse_workout_doc(...)` behavior
- changing existing REST response shapes
- expanding public domain workout models to fully represent pace, HR, LTHR, cadence, and metadata
- changing `planned_workout` parsing as the primary implementation target

## Current Context

The repo currently has two different parsers:
- a lenient `parse_workout_doc(...)` under `src/domain/intervals/workout/parser.rs`
- a strict `parse_planned_workout(...)` under `src/domain/intervals/planned_workout/parser.rs`

`parse_workout_doc(...)` is infallible today and is consumed by REST mapping and training-context flows. That behavior is observable and must remain stable during the PoC.

## Recommended Architecture

Use a shadow-parser design:
- keep the current `parse_workout_doc(...)` as the source of behavior for the application
- add a new internal Pest parser that is a pure function:
  - `fn parse_workout_ast(input: &str) -> Result<WorkoutAst, WorkoutPestParseError>`
- map the AST into the existing `ParsedWorkoutDoc` model only for comparison and persistence
- never let a Pest parser failure block inbound or outbound Intervals flows

This is the smallest correct change and matches the repo rules:
- clear boundaries
- no Mongo leakage into domain parser logic
- no panic on malformed input
- non-breaking integration

## Module Layout

Parser internals live under the workout domain area:
- `src/domain/intervals/workout/pest_parser/mod.rs`
- `src/domain/intervals/workout/pest_parser/workout.pest`
- `src/domain/intervals/workout/pest_parser/ast.rs`
- `src/domain/intervals/workout/pest_parser/error.rs`
- `src/domain/intervals/workout/pest_parser/parser.rs`
- `src/domain/intervals/workout/pest_parser/mapping.rs`

PoC record types and repository port:
- `src/domain/intervals/pest_parser_poc.rs`

Mongo adapter:
- `src/adapters/mongo/pest_parser_poc_workouts.rs`

## AST Design

The AST is internal and intentionally richer than the current public `ParsedWorkoutDoc` model.

Proposed core shapes:
- `WorkoutAst { items: Vec<WorkoutItem> }`
- `WorkoutItem::Step(WorkoutStepAst)`
- `WorkoutItem::RepeatBlock(RepeatBlockAst)`
- `RepeatBlockAst { title: Option<String>, count: usize, steps: Vec<WorkoutStepAst> }`
- `WorkoutStepAst { cue: Option<String>, amount: StepAmount, kind: StepKind, target: Option<ParserTarget>, cadence: Option<CadenceRange>, text: Option<String> }`

Internal enums:
- `StepAmount::{Time(Vec<TimePart>), Distance { value: f64, unit: DistanceUnit }}`
- `StepKind::{Steady, Ramp, FreeRide}`
- `ParserTarget::{PercentFtp, Watts, PercentHr, PercentLthr, Pace, Zone}`

This keeps parser semantics local and avoids premature public-model expansion.

## Grammar Direction

The `.pest` grammar should support the subset needed by the PoC:
- simple steps
- repeat headers with child steps
- time and distance amounts
- FTP percent targets and ranges
- watt targets and ranges
- HR and LTHR targets
- pace targets
- zone targets
- cadence metadata
- `text="..."` metadata
- ramp and freeride modifiers

Important parsing conventions:
- `m` means minutes
- meters should prefer `mtr` in parsing examples to avoid ambiguity
- warmup, cooldown, recovery are semantic cue labels, not grammar keywords
- malformed input must return a structured parser error, never panic

## Proposed Grammar Structure

Top-level:
- `workout = { SOI ~ line* ~ EOI }`
- `line = _{ blank_line | repeat_header_line | bullet_step_line | text_line }`

Repeat blocks:
- `repeat_header_line = { indent? ~ repeat_title? ~ repeat_count ~ line_end }`
- `repeat_count = { int ~ "x" }`
- `repeat_title = { (!repeat_count ~ !line_end ~ ANY)+ }`

Step lines:
- `bullet_step_line = { indent? ~ "-" ~ ws* ~ step_body ~ line_end }`
- `step_body = { cue_prefix? ~ amount ~ ws+ ~ step_modifier? ~ ws* ~ target? ~ ws* ~ cadence? ~ ws* ~ metadata* }`

Units:
- `amount = { time_amount | distance_amount }`
- `time_amount = { time_part+ }`
- `time_part = { number ~ ("h" | "hr" | "hrs" | "m" | "min" | "mins" | "s" | "sec" | "secs" | "'" | "\"") }`
- `distance_amount = { number ~ ("mtr" | "km" | "mi" | "m") }`

Targets:
- `target = _{ ftp_target | watts_target | hr_target | lthr_target | pace_target | zone_target }`
- `ftp_target = { number ~ ("-" ~ number)? ~ "%" }`
- `watts_target = { int ~ ("-" ~ int)? ~ ^"w" }`
- `hr_target = { (number ~ ("-" ~ number)? ~ "%" | zone_expr) ~ ws* ~ ^"HR" }`
- `lthr_target = { (number ~ ("-" ~ number)? ~ "%" | zone_expr) ~ ws* ~ ^"LTHR" }`
- `pace_target = { (pace_value | number ~ ("-" ~ number)? ~ "%" | zone_expr) ~ ws* ~ ^"Pace" }`
- `zone_target = { zone_expr }`
- `zone_expr = { ^"Z" ~ int }`

Metadata:
- `cadence = { cadence_value ~ ^"rpm" }`
- `cadence_value = { int ~ ("-" ~ int)? }`
- `metadata = { text_metadata }`
- `text_metadata = { ^"text" ~ "=" ~ quoted_string }`

## Mapping Policy

The AST-to-`ParsedWorkoutDoc` mapping is intentionally lossy where the public model is narrower.

Preserve where possible:
- repeat counts
n- duration seconds for time-based steps
- zone IDs
- FTP percent min/max values
- canonical normalized step definitions

Current model projection rules:
- `% FTP` maps directly to percent bounds
- `Z1..Z7` maps through the existing percent fallback logic
- unsupported target families such as pace, HR, and LTHR can remain visible in normalized text but do not need a fake percent projection unless the current behavior already has a clear equivalent
- cadence and `text="..."` remain represented in normalized text and persisted PoC payload, not in the public `ParsedWorkoutDoc` fields

## PoC Persistence

Use a dedicated Mongo collection:
- `pest_parser_poc_workout`

Persist both successful and failed parse attempts.

Each record represents one parser observation.

Proposed fields:
- `user_id`
- `direction`
- `operation`
- `source_ref`
- `source_text`
- `parser_version`
- `parsed_at_epoch_seconds`
- `status`
- `normalized_workout`
- `parsed_payload`
- `legacy_projection`
- `error_message`
- `error_kind`
- `intervals_event_id`
- `http_sync_status`

Field semantics:
- `status = "parsed" | "failed"`
- `direction = "inbound" | "outbound"`
- `operation = "list_events" | "get_event" | "create_event" | "update_event"`
- `normalized_workout` is set only on success
- `parsed_payload` is set only on success
- `error_message` and `error_kind` are set only on failure
- `legacy_projection` is optional and used for comparison against the current parser result

Recommended indexes:
- `{ user_id: 1, parsed_at_epoch_seconds: -1 }`
- `{ status: 1, parsed_at_epoch_seconds: -1 }`
- `{ source_type: 1, source_ref: 1, parsed_at_epoch_seconds: -1 }`

## Integration Points

Persistence should happen only at the Intervals adapter boundary, not inside the public parser function itself.

Inbound reads:
- `list_events(...)` in `src/adapters/intervals_icu/client/api.rs`
- optionally `get_event(...)` as the single-event inbound path

Outbound pushes:
- `create_event(...)` in `src/adapters/intervals_icu/client/api.rs`
- `update_event(...)` in `src/adapters/intervals_icu/client/api.rs`

Reasoning:
- this matches the requested trigger semantics exactly
- it avoids duplicate persistence from every internal call site using `parse_workout_doc(...)`
- the parser remains pure and side-effect free

## Safety Rules

Non-negotiable behavior:
- parser function is pure
- malformed input returns `Err`, never panics
- Mongo persistence failures log with `tracing::error!` and do not break main Intervals flows
- parser failures log and persist a failed record but do not block inbound or outbound Intervals operations
- current public `parse_workout_doc(...)` signature remains unchanged

## Testing Strategy

Tests must cover:
- simple workout parsing
- repeat block parsing
- running workout pace syntax
- HR and LTHR syntax
- ramp plus cadence plus `text="..."`
- malformed syntax returning structured errors without panic
- observation helper producing parsed and failed records
- Mongo document persistence for both parsed and failed states
- inbound `list_events` persistence behavior
- outbound `create_event` and `update_event` persistence behavior
- existing Intervals flows remaining non-breaking

Relevant existing suites to keep green:
- `tests/intervals_workout_analysis.rs`
- `tests/intervals_rest/...`
- sync-related tests around calendar/Intervals integration

## Open Constraints Confirmed

Confirmed decisions from the discussion:
- primary target is `workout_doc`
- persist both successful and failed parse attempts
- trigger persistence on every read from Intervals
- trigger persistence on every push/sync from the app to Intervals
- implement in the current workspace, not a worktree

## Summary

The PoC will add a new Pest parser beside the current `workout_doc` parser, keep the existing behavior stable, and persist parser observations into `pest_parser_poc_workout` at the Intervals adapter boundary for both inbound and outbound flows. This provides a safe path to evaluate grammar coverage and failure patterns before any later switch of production behavior.