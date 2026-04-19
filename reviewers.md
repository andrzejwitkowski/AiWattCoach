# Reviewers Log

This file records fixes made in response to review feedback so similar PR and review mistakes are less likely to repeat.

Read this file before planning and before implementation.

## How To Use

- Scan the newest entries first.
- Focus on entries that match the current task area, failure mode, or review pattern.
- When you apply a fix based on feedback from the user, Copilot, or CodeRabbit, add a new entry immediately after the fix.

## Entry Format

- Date: `YYYY-MM-DD`
- Source: user | Copilot | CodeRabbit
- Scope: file, feature, or review area
- Problem: what was wrong or missing
- Fix: what changed to address it
- Prevention: what to check next time before sending work for review

## Entries

### 2026-04-19 | user | backfill refactor readability

- Problem: test doubles used tuple-shaped call records that obscured field meaning, `backfill_missing_metrics` stayed too monolithic, and backfill tests were still too large to navigate comfortably.
- Fix: replaced tuple call records with named structs, split metrics backfill orchestration into explicit helper phases, and divided backfill tests into `details`, `metrics`, and shared `support` modules.
- Prevention: when a test helper or orchestration path starts relying on positional values or exceeds a few logical phases, refactor immediately into named data structures and concern-based files before adding more behavior.

### 2026-04-19 | user | completed workout metrics backfill

- Problem: the new metrics backfill used the stale completed-workout date to choose `recomputed_from`, which could miss earlier snapshots if the Intervals activity import corrected the activity date.
- Fix: changed the backfill flow to derive `recomputed_from` from `detailed_activity.start_date_local` after fetching the refreshed Intervals payload.
- Prevention: for any batch import followed by recompute, confirm that the recompute boundary comes from the final imported source-of-truth record, not the pre-import local copy.

### 2026-04-19 | user | agent process docs

- Problem: the repo instructions did not include a durable review-fix loop, so repeated PR and review mistakes were not being logged in a reusable place.
- Fix: created `reviewers.md`, added the review-fix loop to `AGENTS.md`, and added the reusable lesson to `tasks/lessons.md`.
- Prevention: before writing a plan or implementing changes, read `reviewers.md` and check whether the current task repeats a known review pattern.
