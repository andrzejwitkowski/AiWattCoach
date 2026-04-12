# AGENTS Karpathy Guardrails Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a small repo-tracked section to `AGENTS.md` that adapts the upstream Andrej Karpathy coding guardrails for OpenCode in `AiWattCoach`.

**Architecture:** Keep the change documentation-only and localized to `AGENTS.md`. Add one new section near the top-level workflow guidance so the rules are visible early without rewriting existing project constraints.

**Tech Stack:** Markdown, repository agent instructions

---

### Task 1: Add the new guidance section

**Files:**
- Modify: `AGENTS.md`

**Step 1: Draft the new section**

- Add `## Karpathy Guardrails for Agents`.
- Add four concise bullet groups:
  - think before changing
  - simplicity first
  - surgical changes
  - goal-driven verification

**Step 2: Keep the wording repo-compatible**

- Preserve existing OpenCode and repo-specific terminology.
- Avoid references to `CLAUDE.md`.
- Do not duplicate large existing sections.

**Step 3: Verify readability**

Run: `git diff -- AGENTS.md`

Expected: One focused documentation diff with the new section only.

### Task 2: Verify the final change set

**Files:**
- Modify: `AGENTS.md`
- Create: `docs/plans/2026-04-12-agents-karpathy-guardrails-design.md`
- Create: `docs/plans/2026-04-12-agents-karpathy-guardrails-implementation-plan.md`

**Step 1: Inspect the final diff**

Run: `git diff -- AGENTS.md docs/plans/2026-04-12-agents-karpathy-guardrails-design.md docs/plans/2026-04-12-agents-karpathy-guardrails-implementation-plan.md`

Expected: Only the intended docs changes appear.

**Step 2: Stop after verification**

- Do not widen scope into other instruction files.
- Do not edit unrelated docs.
