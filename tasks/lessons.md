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
