# Planned Completed Link Match Sources

This note explains what `PlannedCompletedWorkoutLinkMatchSource` means and when each value should be used.

Relevant model:

- `src/domain/planned_completed_links/model.rs`

```rust
pub enum PlannedCompletedWorkoutLinkMatchSource {
    Explicit,
    Token,
    Heuristic,
}
```

## Purpose

`match_source` records why the system believes a planned workout and completed workout represent the same logical workout.

It is not just metadata for curiosity.

It matters because:

- token-based matches are stronger than heuristic guesses
- explicit matches are stronger than both
- future repair or migration code can trust or rank links differently depending on how they were created

## Current State

Today the implemented import path persists `Token` matches when a completed workout import resolves a `planned_workout_id` and upserts a link row.

Current write path:

- `src/domain/external_sync/import/mod.rs`

At the moment:

- `Token` is actively written in production code
- `Explicit` and `Heuristic` already exist in the model and Mongo mapping
- but they are currently semantic slots for current or future link creation paths, not yet fully exercised by the import codepath

So this document describes both:

- the current implemented `Token` behavior
- the intended meaning of all three enum values

## Scenario 1: Explicit

Use `Explicit` when the link is set by a direct authoritative decision instead of inferred matching.

Examples:

- a future workflow where the user manually confirms that completed workout `X` belongs to planned workout `Y`
- a future command that directly writes the pair using known canonical ids
- a future provider payload that includes an authoritative planned-workout reference and the system chooses to treat that as a direct link rather than an inferred token match

Meaning:

- the system did not have to guess
- the link came from an explicit authoritative source

Trust level:

- highest

## Scenario 2: Token

Use `Token` when the system finds the `[AIWATTCOACH:pw=<token>]` marker and resolves the planned workout from `planned_workout_tokens`.

Current flow:

1. a planned workout is synced outward and gets a marker embedded in description text
2. a completed activity comes back from the provider
3. import mapping extracts marker candidates from provider payload fields such as:
   - `external_id`
   - `description`
   - `name`
4. `extract_planned_workout_marker(...)` finds a valid marker token
5. the token repository resolves that token to a `planned_workout_id`
6. import persists the canonical completed workout with that `planned_workout_id`
7. import upserts `planned_completed_links` with `match_source = Token`

Relevant files:

- `src/domain/planned_workout_tokens/token.rs`
- `src/domain/external_sync/import/mod.rs`

Meaning:

- the link is inferred from an intentional marker created by AiWattCoach
- this is not a free-text guess
- it is a strong match because the marker was deliberately planted for correlation

Trust level:

- high

## Scenario 3: Heuristic

Use `Heuristic` when the link is chosen by fallback matching logic rather than an explicit authoritative reference or marker token.

The intended example in this codebase is same-day fallback matching:

- no token resolves
- there is exactly one planned workout on the completed workout day
- the system links the completed workout to that one planned workout because there is no competing candidate

Meaning:

- the system made the best available inference from local context
- the link may be correct, but it is weaker than `Explicit` or `Token`

Trust level:

- lowest of the three

## Practical Interpretation

From strongest to weakest:

1. `Explicit`
2. `Token`
3. `Heuristic`

If future reconciliation or repair logic ever needs to choose between competing links, this should be the default precedence order unless a stronger product rule overrides it.

## Why This Matters

The enum exists so the codebase can keep two things separate:

- the fact that a link exists
- the reason the link was considered valid

That gives us better options later for:

- migrations
- confidence ranking
- conflict resolution
- auditability
- targeted cleanup of weakly matched links
