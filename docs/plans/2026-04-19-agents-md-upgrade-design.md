# AGENTS.md Upgrade Design

**Goal:** Add a standalone section to `AGENTS.md` that captures the requested agent operating rules from the screenshot without restructuring existing guidance.

**Approach:** Keep the change documentation-only and minimal. Add one new section after `Karpathy Guardrails for Agents` so the new rules stay grouped with agent operating guidance while avoiding edits across multiple existing sections.

## Decision

- Add one new section named `Agent Execution Rules`.
- Keep the new guidance grouped into four subsections:
  - `Self-Improvement Loop`
  - `Verification Before Done`
  - `Demand Elegance (Balanced)`
  - `Autonomous Bug Fixing`
- Write the section in English and keep the phrasing aligned with the surrounding rules.

## Why This Shape

- It keeps the diff small and easy to review.
- It avoids scattering one user request across many existing sections.
- It preserves the current structure of `AGENTS.md` while making the new operating rules explicit.
