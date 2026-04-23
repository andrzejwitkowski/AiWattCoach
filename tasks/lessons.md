# Lessons

## Review Fix Logging Loop

- When I implement a fix based on feedback from the user, Copilot, or CodeRabbit, I must record it in `reviewers.md`.
- Each `reviewers.md` entry must state both the problem that was identified and the fix that was applied.
- The purpose of this loop is to reduce repeated PR and review mistakes over time.
- I must read `reviewers.md` before writing a plan and before starting implementation work.

## Backfill Recompute Ranges

- When a backfill or reimport operation can change canonical record dates, I must derive recompute ranges from the refreshed upstream payload, not from the stale local record.
- Before finalizing batch recompute logic, verify that the chosen `oldest_changed` date still covers records whose timestamps may be corrected during import.

## Test Doubles And Shapes

- In tests, avoid tuple aliases for multi-field call records when the field meaning matters. Use named structs or named sub-structs so assertions stay self-explanatory.
- When a function grows past a few distinct phases, split it into small helpers named after each phase instead of leaving one long orchestration block.
- When a test file grows large, split it by behavior group and extract shared fakes/fixtures into a local `support` module.

## Small Review Fixes

- In response/body mappers, decode a byte payload to UTF-8 once and reuse the borrowed text across classification and logging helpers instead of repeating `from_utf8(...)` work.
- If parsed JSON string data is only compared against a static literal, keep it borrowed as `&str` and compare in place instead of allocating an owned `String` first.

## Release Workflow Reliability

- When a GitHub Actions workflow grows bespoke version/tagging logic, extract it into a repository script with unit tests instead of leaving the logic inline in YAML.
- If a git tag is intended to mean "artifact is available", publish the artifact first and push the tag only after the publish step succeeds.
- Any workflow that uses `Swatinem/rust-cache` or Docker Buildx `cache-to/from: type=gha` must keep `actions` permission enabled explicitly when permissions are restricted.
- Scripts that shell out to `docker build` or similar filesystem-sensitive commands must anchor their working paths to the repo/script location instead of assuming the caller launched them from the repo root.
