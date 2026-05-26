# Training Plan JSON Envelope Design

**Goal:** Change training-plan LLM generation and correction from raw text responses to a schema-driven JSON envelope so the parser consumes only a dedicated `plan` field, while optional model commentary is preserved separately in `description` for debugging and operations visibility.

## Scope

In scope:
- initial training-plan generation response contract
- correction response contract
- prompt/schema changes for both phases
- response parsing changes for both phases
- durable storage of optional `description` text for operation/debug visibility
- structured logging of `description` metadata
- focused tests for prompt/schema and response parsing behavior

Out of scope:
- changing the underlying workout-builder grammar inside `plan`
- changing snapshot/projection semantics
- changing the 14-day contiguous validation rule
- adding new REST response fields for training-plan generation
- replacing the existing planned-workout parser with a fully structured day array model

## Current Context

Today the training-plan generator asks the LLM to return raw workout text directly:
- `src/adapters/llm/training_plan_generator.rs` builds a text-only prompt
- `require_assistant_text(...)` extracts raw assistant text
- `TrainingPlanPhaseOutput` carries `raw_response: String`
- `src/domain/training_plan/service/parsing.rs` parses that raw text into day blocks

This means the model can still break the contract by adding commentary before the first dated section even when the prompt forbids it.

The repo already has a stronger pattern for structured LLM output in workout summary coaching:
- `src/domain/workout_summary/coach_output.rs` defines a schema-backed JSON contract
- `src/adapters/llm/workout_summary_coach.rs` embeds that schema in the system prompt
- downstream parsing consumes the structured JSON payload instead of trusting free-form assistant text

That is the right pattern to reuse here.

## Approved Contract

The training-plan LLM should return this JSON envelope for both initial generation and correction:

```json
{
  "plan": "2026-05-27\nRest Day\n\n2026-05-28\nEndurance\n- 45m 65%",
  "description": "Optional explanation, rationale, or commentary from the model."
}
```

Field rules:
- `plan`
  - required
  - string only
  - must contain only parser-friendly dated workout text
  - is the only field used for `parse_window(...)`
- `description`
  - optional
  - string when present
  - may contain free text, rationale, or commentary
  - must not be parsed as workout text
  - must be stored durably for debug/operations visibility

Unknown fields should be rejected at parse time by serde/schema contract.

## Recommended Architecture

Use a schema-backed envelope exactly like the workout-summary coach flow.

### Domain contract

Add a small training-plan response envelope contract under the training-plan domain area, for example:
- `src/domain/training_plan/llm_output.rs`

Proposed responsibilities:
- define the envelope struct
- derive `Serialize`, `Deserialize`, and `JsonSchema`
- expose helper functions:
  - parse initial/correction envelope payload
  - render JSON schema for prompt inclusion

Suggested shape:
- `TrainingPlanLlmEnvelope { plan: String, description: Option<String> }`
- `#[serde(deny_unknown_fields)]`
- schema-level `additionalProperties: false`

This keeps the LLM contract explicit and versionable without leaking provider concerns into the domain service.

### Adapter behavior

Update `src/adapters/llm/training_plan_generator.rs` so both initial and correction prompts:
- require JSON-only output matching the training-plan schema
- embed the generated JSON schema in the system prompt
- continue to keep the workout-builder grammar rules, but scoped specifically to the `plan` field

Replace `require_assistant_text(...)` usage in this adapter with a helper that:
- extracts assistant text
- parses it as `TrainingPlanLlmEnvelope`
- returns both `plan` and `description`
- maps JSON/schema failures to `TrainingPlanError::Unavailable` or `Validation` consistently with current adapter error boundaries

### Service behavior

The training-plan domain service should continue to operate on workout text only, but from the envelope field rather than the whole assistant response.

That means:
- parse `envelope.plan` with existing `parse_window(...)`
- ignore `description` for planning logic
- preserve `description` in operation/debug state

This is the smallest correct change because it avoids rewriting the core planning parser and correction loop.

## Persistence Changes

`TrainingPlanGenerationOperation` should gain durable fields for the optional description text from both phases.

Recommended fields:
- `raw_plan_description: Option<String>`
- `raw_correction_description: Option<String>`

Rationale:
- keep parity with existing `raw_plan_response` / `raw_correction_response`
- make the fields obviously debug-oriented
- preserve phase separation

These fields should be persisted in Mongo along with existing raw response/tool-loop fields.

This allows operators to inspect:
- the exact parser-consumed `plan`
- the model's separate commentary in `description`

## Logging Requirements

When receiving an initial or correction envelope, log structured metadata about `description`.

Recommended log fields:
- `operation_key`
- `phase` (`initial_generation` or `correction`)
- `has_description`
- `description_chars`
- `plan_chars`
- `description_preview`

Recommended preview policy:
- log only a short preview, not the full text
- keep preview length small and bounded
- trim whitespace

This gives operators visibility without turning logs into raw transcript dumps.

## Prompt Design

The new training-plan prompt should follow the workout-summary schema pattern.

### Initial generation system prompt

It should still say:
- generate a 14-day internal cycling plan window
- follow backend-supported grammar
- respect planning guidelines and availability

But the output instructions should change from "raw workout text only" to something like:
- return JSON only matching the training-plan response schema
- put only parser-friendly dated workout text in `plan`
- put any rationale or extra commentary in `description`
- do not place commentary inside `plan`

### Correction system prompt

It should similarly say:
- return JSON only matching the same schema
- `plan` must contain only corrected dated sections for invalid dates
- keep valid days untouched
- any explanation belongs in `description`

### User prompt

The user prompt can stay conceptually similar, but should describe the `plan` field explicitly instead of asking for direct raw text.

## Parsing Policy

Parsing policy should be strict and simple:
- first parse envelope JSON
- then parse `envelope.plan` with current text parser
- never parse `description`
- reject unknown JSON fields
- reject missing or empty `plan`

This eliminates the main class of failures where explanatory text pollutes the plan body.

## Correction Flow Compatibility

Use the same envelope contract for correction responses.

That keeps:
- one prompt contract
- one parsing helper
- one persistence shape
- one logging approach

It also prevents a split-brain contract where initial generation is structured but correction still relies on raw text.

## File Layout

Recommended touched files:
- `src/domain/training_plan/llm_output.rs`
- `src/domain/training_plan/model.rs`
- `src/domain/training_plan/mod.rs`
- `src/adapters/llm/training_plan_generator.rs`
- `src/adapters/mongo/training_plan_generation_operations.rs` or the current Mongo operation adapter file(s)
- `src/domain/training_plan/service/mod.rs`
- `tests/llm_adapters/training_plan.rs`
- `tests/training_plan_mongo.rs`
- `tests/training_plan_service/validation.rs`

Potentially also:
- operation mapper tests or Mongo round-trip tests for the new description fields

## Error Handling Rules

Keep adapter/domain boundaries clear:
- provider response text extraction stays in the LLM adapter
- JSON envelope parsing belongs with the training-plan output contract helper
- workout text parsing stays in the training-plan service parser

Failure modes:
- malformed JSON envelope: adapter-level invalid response
- missing `plan`: adapter-level invalid response
- invalid workout grammar inside `plan`: existing training-plan validation/correction path
- invalid 14-day contiguity: existing snapshot validation path

## Testing Strategy

Add or update tests for all of the following:

### Prompt and schema tests
- initial prompt includes `training_plan_response_schema=`
- initial prompt instructs JSON-only output
- initial prompt says commentary belongs in `description`
- correction prompt includes the same schema and rules

### Adapter parsing tests
- valid envelope with `plan` only parses successfully
- valid envelope with `plan` plus `description` parses successfully
- malformed JSON fails clearly
- unknown field fails clearly
- missing `plan` fails clearly

### Service behavior tests
- initial generation uses `plan` only and ignores `description`
- correction flow uses corrected `plan` only and preserves `description`
- commentary in `description` does not trigger `content before first date header`

### Persistence tests
- Mongo round-trip preserves `raw_plan_description`
- Mongo round-trip preserves `raw_correction_description`

### Logging tests
- if this repo has existing targeted observability/logging tests for training-plan generator logging, add or extend one to verify bounded description metadata logging
- if not, keep logging assertions minimal and local to existing conventions

## Trade-offs Considered

### Option 1: `plan` plus optional `description` in JSON envelope

Pros:
- solves commentary contamination directly
- matches the repo's existing schema-based workout-summary pattern
- minimal change to core parser/service logic
- preserves model rationale for debugging

Cons:
- requires operation schema changes and Mongo mapping updates
- requires prompt/test updates in both initial and correction flows

### Option 2: `plan` only JSON envelope

Pros:
- smallest envelope shape
- simplest parser contract

Cons:
- loses sanctioned place for model commentary
- does not satisfy the approved requirement to preserve commentary in operation/debug state

### Option 3: fully structured JSON days instead of workout text

Pros:
- strongest syntactic separation
- avoids a second text parser layer eventually

Cons:
- overbuilt for current needs
- duplicates the existing workout-builder grammar in a second format
- much larger migration and test surface

Approved choice:
- Option 1

## Open Decisions Resolved

Confirmed from the discussion:
- use a JSON envelope instead of raw text
- keep `plan` as parser-friendly workout text, not a full day-array model
- include optional `description`
- persist `description` in operation/debug state
- log description metadata for operator visibility
- apply the same contract to initial generation and correction

## Summary

The training-plan LLM should switch from free-form raw text output to a schema-backed JSON envelope with a required `plan` field and an optional `description` field. The backend should parse only `plan`, preserve `description` in durable operation state, and log bounded metadata about it. This reuses the repo's proven workout-summary JSON-schema pattern while keeping the existing workout-builder parser and training-plan correction flow largely unchanged.
