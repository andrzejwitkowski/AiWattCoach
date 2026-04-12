# AGENTS Karpathy Guardrails Design

## Goal

Add a compact set of behavioral guardrails to `AGENTS.md`, adapted from `forrestchang/andrej-karpathy-skills`, so OpenCode follows the same caution-first mindset inside `AiWattCoach`.

## Recommended Approach

Add one dedicated section to `AGENTS.md` instead of scattering the guidance across existing sections. This keeps the source of truth easy to find, preserves the current repo-specific instructions, and makes the imported guidance clearly identifiable as an additional layer rather than a rewrite of the whole file.

## Scope

- Add a new section named `Karpathy Guardrails for Agents`.
- Keep the section short and project-compatible.
- Translate the upstream `CLAUDE.md` ideas into OpenCode-friendly wording.
- Avoid duplicating rules that already exist unless the new wording adds a useful emphasis.

## Section Structure

- `Think Before Changing` for ambiguity, assumptions, and surfacing tradeoffs.
- `Simplicity First` for minimal implementations and avoiding speculative abstractions.
- `Surgical Changes` for small diffs and avoiding unrelated cleanup.
- `Goal-Driven Verification` for turning requests into verifiable outcomes.

## Constraints

- Do not mention `CLAUDE.md` as the integration target.
- Do not introduce instructions that conflict with the repo's existing verification and architecture rules.
- Keep the diff minimal and readable.

## Verification

- Read the updated `AGENTS.md` section for tone and consistency.
- Run `git diff -- AGENTS.md docs/plans/2026-04-12-agents-karpathy-guardrails-design.md` to verify the change set is limited to the intended files.
