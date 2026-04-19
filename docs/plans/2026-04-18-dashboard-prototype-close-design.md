# Dashboard Prototype-Close Design

**Goal:** Polish the existing training load dashboard so the `/app` screen looks materially closer to the approved issue `#103` prototype while keeping the current snapshot-backed data model and explicitly omitting the `Plan Deload Week` CTA.

**Architecture:** Keep the current frontend data flow unchanged: `AppHomePage` still loads the dashboard payload and renders `TrainingLoadReport`. Limit the work to the existing dashboard feature components so the change stays presentation-only, with no REST, domain, or schema changes.

**Tech Stack:** React, TypeScript, Tailwind CSS, existing SVG chart rendering, Vitest, Testing Library

---

## Scope

- Keep routing, API calls, and Zod parsing unchanged.
- Keep the current `90 DAYS`, `SEASON`, and `ALL TIME` controls and behavior unchanged.
- Make the top section, charts, and right-hand panel visually closer to the issue prototype.
- Preserve the removed deload CTA by using informational insight content only.

## Design Direction

### Header and Range Controls

- Shift the screen toward the prototype's editorial hierarchy:
  - tighter uppercase eyebrow
  - stronger headline treatment
  - short supporting copy
- Restyle the range switch as a darker segmented control integrated into the same visual language as the cards.

### Chart Cards

- Keep the current SVG chart approach instead of introducing a charting library.
- Restyle both cards with:
  - darker surfaces
  - stronger legends for CTL, ATL, and TSB
  - clearer grid framing
  - y-axis labels
  - bottom date markers
  - a visible latest-point or today indicator derived from the newest snapshot
- For TSB, add clearer zone framing so `freshness_peak`, `optimal_training`, and `high_risk` read more like the prototype.

### Right-Hand Panel

- Replace the current single summary card feel with a prototype-close two-part treatment:
  - a TSB explainer block with zone descriptions
  - a coach insight block with stronger headline and current metrics
- Keep this panel purely informational.
- Do not add any action button or new workflow.

## Testing

- Update frontend tests to assert the new visible structure and copy that distinguishes the prototype-close layout.
- Run targeted frontend tests first, then broader frontend verification.
