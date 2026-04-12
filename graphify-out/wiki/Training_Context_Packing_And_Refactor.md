# Training Context Packing And Refactor

> 65 nodes · cohesion 0.05

## Key Concepts

- **model.rs** (19 connections) — `src\domain\training_context\model.rs`
- **2026-04-06-backend-slice-size-refactor-design.md** (8 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **2026-04-06-backend-slice-size-refactor.md** (8 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **2026-04-04-power-compression-llm.md** (8 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- **workout_summary_coach.rs** (8 connections) — `src\adapters\llm\workout_summary_coach.rs`
- **Split bulky current-slice shared helpers and integration suites into directory-based test modules** (7 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Reduce the training-plan and workout-summary backend slice by splitting oversized files without changing behavior** (6 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Keep behavior, public APIs, durable workflow ordering, idempotency behavior, and adapter boundaries unchanged** (6 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Run broader backend verification after the refactor** (6 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **Split bulky workout-summary shared helpers and remaining oversized current-slice tests** (6 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **.reply()** (6 connections) — `src\adapters\llm\workout_summary_coach.rs`
- **Split Mongo workout-summary adapter tests first and helper concerns only if still oversized** (5 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Split training_context packing into focused packing/ siblings** (5 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Split training_context service into focused service/ siblings** (5 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Split workout_summary service into focused service/ siblings** (5 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- **Update the workout coach prompt with power-compression semantics** (5 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- **Run full formatting, lint, Rust tests, and repo verification** (5 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- **RecentWorkoutContext.compressed_power_levels: Vec<String>** (5 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- **workout_summary_flow.rs** (5 connections) — `tests\llm_rest\workout_summary_flow.rs`
- **Split the Mongo workout-summary adapter by moving tests first and helpers only if needed** (4 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **Split training_context packing into focused payload and rendering modules** (4 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **Split training_context service into focused directory modules** (4 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **Split workout_summary service while keeping WorkoutSummaryService as the entrypoint** (4 connections) — `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- **Update LLM flow assertions to expect pc in live requests** (4 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- **Pack pc instead of p5** (4 connections) — `docs\plans\2026-04-04-power-compression-llm.md`
- *... and 40 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `docs\plans\2026-04-04-power-compression-llm.md`
- `docs\plans\2026-04-06-backend-slice-size-refactor-design.md`
- `docs\plans\2026-04-06-backend-slice-size-refactor.md`
- `src\adapters\llm\workout_summary_coach.rs`
- `src\domain\training_context\model.rs`
- `tests\llm_rest\workout_summary_flow.rs`

## Audit Trail

- EXTRACTED: 215 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*