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

### 2026-04-27 | user | planned workout calendar duration parsing

- Problem: `parse_workout_doc(...)` treated canonical planned-workout repeat headers like `Main Set 2x` as standalone lines, so the following step lines were not expanded as a repeated block and calendar planned-workout summaries undercounted total duration.
- Fix: taught the workout parser to recognize canonical repeat-header lines without inline durations, expand the following contiguous timed steps as a repeated block when building segments, and added a regression test for the full repeated duration/segment labels.
- Prevention: when one parser consumes text emitted by another canonical serializer, add regression coverage for structural constructs like repeat headers instead of assuming a flat line-by-line parser preserves grouped semantics.

### 2026-04-24 | CodeRabbit/Copilot | PR #141 Wahoo OAuth review follow-up

- Problem: the first Wahoo OAuth connect version duplicated security-sensitive `returnTo` sanitization, used a misleading `NotConfigured` error for missing per-user Wahoo credentials, built the live reqwest client with `.expect(...)`, leaked Wahoo tokens through Mongo `Debug`, discarded all token-endpoint error detail, and accepted callback state consumption without binding it to the authenticated app user.
- Fix: extracted shared `returnTo` sanitization into `src/domain/return_to.rs` and loosened it to allow timestamp-style query parameters, renamed the per-user credential error to `NotConnected`, propagated reqwest client build errors from `main`, redacted `WahooDocument` token fields in `Debug`, summarized Wahoo OAuth error payloads via parsed `error`/`error_description` or a size/hash fallback, and changed the callback flow to require the authenticated user and consume connect state scoped to that user.
- Prevention: when adding another OAuth-style integration, reuse shared redirect sanitization, keep server-config vs per-user credential errors distinct, never use panic-style startup wiring for HTTP clients, redact adapter persistence models as well as domain models, preserve bounded upstream error detail, and make callback state consumption explicitly user-bound if the browser callback returns to an authenticated app session.

### 2026-04-24 | user | Wahoo OAuth endpoint/scope configuration

- Problem: the first Wahoo OAuth client version kept authorize URL, token URL, and scope as hard-coded adapter constants, so the review request to make them env-configurable with defaults was not addressed through the repo's normal config path.
- Fix: added optional `WAHOO_OAUTH_AUTHORIZE_URL`, `WAHOO_OAUTH_TOKEN_URL`, and `WAHOO_OAUTH_SCOPE` settings with Wahoo defaults in centralized settings parsing, wired those values into `WahooOAuthClient`, updated `.env.example`, and added focused settings tests for both default and override behavior.
- Prevention: when a review asks for env-driven behavior, first check whether the repo already has a startup settings seam and implement the override there instead of adding ad hoc environment reads in leaf adapters.

### 2026-04-23 | Copilot | PR #138 Intervals Strava 422 logging follow-up

- Problem: the first version of the Intervals Strava-422 classifier decoded the same response body to UTF-8 twice inside `map_error_response_from_logged_response(...)` and allocated a temporary `String` just to compare the parsed `error` field with a static message.
- Fix: reused one decoded `Option<&str>` for both the known-422 classifier and the hashed log-summary path, and changed the parsed JSON `error` extraction to stay borrowed as `&str` so the comparison avoids an unnecessary allocation.
- Prevention: when a review fix adds lightweight response classification, reread the hot-path helper for duplicate decoding/parsing work and for avoidable temporary allocations before sending it back for review.

### 2026-04-22 | CodeRabbit/Copilot | PR #128 release workflow follow-up

- Problem: the first registry-release version left version-resolution logic embedded inline in GitHub Actions without tests, pushed release tags before image publication could fail, kept cache permissions too narrow for `rust-cache` and Buildx `type=gha`, and let the fallback publish script depend on the caller's current working directory.
- Fix: extracted release version resolution into `scripts/resolve-release-version.mjs` with unit tests, refactored the fallback publish helper into testable functions with unit tests and repo-root-based Docker context, moved git tag creation/push to after the image publish step succeeds, and added `actions: write` where the workflow uses GitHub Actions cache APIs.
- Prevention: when a workflow introduces custom versioning or release orchestration, move the logic into a testable script instead of inline bash; if a release tag is meant to imply a deployable artifact, publish the artifact first and push the tag only after success; any workflow using `rust-cache` or Buildx `type=gha` must keep `actions` token permissions explicit; CLI helpers that shell out should anchor filesystem context to known paths instead of assuming the caller's cwd.

### 2026-04-19 | Copilot | admin metrics backfill test coverage

- Problem: the non-admin metrics backfill REST test omitted same-origin headers, so it could return `403` at the CSRF/same-origin guard before reaching `require_admin`.
- Fix: added `Host` and `Origin` headers to the non-admin metrics backfill test so it now exercises the authorization branch intentionally.
- Prevention: when testing authorization behind request-shape guards, satisfy the earlier transport checks first so the test reaches the branch it claims to cover.

### 2026-04-19 | CodeRabbit | metrics backfill selection and observability

- Problem: metrics backfill imported activities whenever any metric existed upstream, even if none of the currently missing fields would be filled, and fetch/import failures were counted without diagnostic context.
- Fix: tightened the metrics backfill gate to require at least one missing field to be provided by the fetched activity, added a regression test for that case, and logged fetch/import failures with structured `warn!` fields.
- Prevention: for partial backfills, compare upstream data against the specific missing local fields before counting an item as enriched, and log batch-processing failures with enough identifiers to debug retries.

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
