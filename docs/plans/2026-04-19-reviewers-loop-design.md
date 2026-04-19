# Reviewers Loop Design

**Goal:** Add durable agent notes for tracking review-driven fixes and require reading that log before planning or implementation.

**Approach:** Create two small markdown files with clear responsibilities. `tasks/lessons.md` stores the reusable lesson as an operating rule, while `reviewers.md` becomes the running log of fixes made in response to review feedback from the user, Copilot, and CodeRabbit.

## Decision

- Create `tasks/lessons.md` for reusable behavioral rules.
- Create `reviewers.md` as the persistent review-fix log.
- Require each review-driven fix entry to include the problem and the applied fix.
- Require reading `reviewers.md` before planning and before implementation.

## Why This Shape

- `tasks/lessons.md` captures the rule once in reusable form.
- `reviewers.md` keeps concrete examples of review mistakes and their fixes.
- The separation keeps the lesson concise while allowing the review log to grow over time.
