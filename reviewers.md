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

### 2026-04-19 | user | agent process docs

- Problem: the repo instructions did not include a durable review-fix loop, so repeated PR and review mistakes were not being logged in a reusable place.
- Fix: created `reviewers.md`, added the review-fix loop to `AGENTS.md`, and added the reusable lesson to `tasks/lessons.md`.
- Prevention: before writing a plan or implementing changes, read `reviewers.md` and check whether the current task repeats a known review pattern.
