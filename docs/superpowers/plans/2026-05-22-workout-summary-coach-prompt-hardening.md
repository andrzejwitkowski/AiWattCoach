# Workout Summary Coach Prompt Hardening Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change the workout-summary coach prompt so the LLM evaluates post-workout execution primarily from `bl`, `pc`, and `c5`, treats aggregate metrics as secondary context only, and uses tools only when those packed signals are insufficient for a confident conclusion.

**Architecture:** Keep the change local to the existing workout-summary LLM adapter and tool guidance layer. Do not change training-context payload shape or orchestration; instead, strengthen the coach’s evidence hierarchy in the base prompt and tool usage guidance, then lock it in with focused prompt-level tests.

**Tech Stack:** Rust, existing LLM adapter prompt assembly, domain LLM tools, Rust unit/integration-style adapter tests.

---

### Scope Summary

This plan should touch only the prompt and prompt-guidance surfaces already used by `WorkoutSummaryChat`, plus focused tests.

Expected code areas:
- `src/adapters/llm/workout_summary_coach.rs`
- `src/domain/llm_tools/mod.rs`
- `tests/llm_adapters/coaching.rs`
- possibly one `src/domain/llm_tools/*` test if guidance text is asserted there
- `reviewers.md`
- `tasks/lessons.md`
- `docs/superpowers/plans/2026-05-22-workout-summary-coach-prompt-hardening.md`
- after implementation: `graphify-out/**` via `./scripts/rebuild_graphify.sh`

Non-goals:
- no backend prefetch of completed workout detail
- no training-context schema change
- no new tools
- no parser/schema changes for coach output

---

### Task 1: Confirm Current Prompt and Guidance Baseline

**Files:**
- Read: `src/adapters/llm/workout_summary_coach.rs`
- Read: `src/domain/llm_tools/mod.rs`
- Read: `tests/llm_adapters/coaching.rs`
- Read: `src/domain/training_context/service/tests/builder/rendering.rs`

- [ ] **Step 1: Re-read the current workout coach system prompt**
  
  Verify the current base prompt in `src/adapters/llm/workout_summary_coach.rs` does all of the following today:
  - describes coaching tone
  - requires JSON-only output
  - mentions packed training context
  - does not yet define an evidence hierarchy for `bl`, `pc`, `c5`
  - does not yet explicitly forbid interval-execution judgments from `NP` / `avg power` / `IF` / `VI` / `TSS` alone

- [ ] **Step 2: Re-read current tool guidance assembly**
  
  Verify in `src/domain/llm_tools/mod.rs`:
  - `with_tool_prompt_guidance(...)` appends tool instructions after the base system prompt
  - current generic rule says “call it instead of guessing”
  - tool guidance is scope-aware and provider-aware
  - `WorkoutSummaryChat` has access to `get_selected_workout` and `selected_workout_power_curve`

- [ ] **Step 3: Reconfirm packed-context semantics**
  
  Reconfirm from `src/domain/training_context/service/tests/builder/rendering.rs` that:
  - `bl` exists in packed context
  - `pc` exists in packed context
  - `c5` exists in packed context
  - full raw per-sample power series is not guaranteed in packed context
  - therefore the prompt must treat `bl` + `pc` + `c5` as primary local evidence, with tools as escalation

- [ ] **Step 4: Reconfirm existing prompt tests**
  
  Identify which current tests in `tests/llm_adapters/coaching.rs` already assert prompt text and can be extended instead of adding redundant new files.

---

### Task 2: Add Failing Prompt-Level Tests For Evidence Hierarchy

**Files:**
- Modify: `tests/llm_adapters/coaching.rs`

- [ ] **Step 1: Add a failing test for primary evidence hierarchy**
  
  Add a test that asserts the built workout-summary system prompt explicitly says execution quality should be judged primarily from `bl`, `pc`, and `c5`.

  Test intent:
  ```rust
  #[tokio::test]
  async fn llm_workout_coach_prioritizes_bl_pc_c5_for_execution_quality() {
      let chat_port = Arc::new(CapturingChatPort::default());
      let coach = LlmWorkoutCoach::new(
          chat_port.clone(),
          Arc::new(FixedGeminiConfigProvider),
          Arc::new(StubTrainingContextBuilder),
          FixedClock,
      );

      coach
          .reply("user-1", &sample_summary(), "How did I do?", None)
          .await
          .unwrap();

      let prompt = &chat_port.requests()[0].system_prompt;
      assert!(prompt.contains("bl"));
      assert!(prompt.contains("pc"));
      assert!(prompt.contains("c5"));
      assert!(prompt.contains("primary evidence"));
  }
  ```

- [ ] **Step 2: Add a failing test for anti-aggregate rule**
  
  Add a test that asserts the prompt explicitly forbids judging interval execution from aggregate metrics alone.

  Test intent:
  ```rust
  #[tokio::test]
  async fn llm_workout_coach_forbids_interval_judgment_from_aggregate_metrics_alone() {
      let chat_port = Arc::new(CapturingChatPort::default());
      let coach = LlmWorkoutCoach::new(
          chat_port.clone(),
          Arc::new(FixedGeminiConfigProvider),
          Arc::new(StubTrainingContextBuilder),
          FixedClock,
      );

      coach
          .reply("user-1", &sample_summary(), "How did I do?", None)
          .await
          .unwrap();

      let prompt = &chat_port.requests()[0].system_prompt;
      assert!(prompt.contains("NP"));
      assert!(prompt.contains("average power"));
      assert!(prompt.contains("must not"));
      assert!(prompt.contains("interval execution"));
  }
  ```

- [ ] **Step 3: Add a failing test for tool escalation rule**
  
  Add a test that asserts the prompt tells the model to escalate to tools only when `bl`/`pc`/`c5` are insufficient.

  Test intent:
  ```rust
  #[tokio::test]
  async fn llm_workout_coach_uses_tools_only_when_bl_pc_c5_are_insufficient() {
      let chat_port = Arc::new(CapturingChatPort::default());
      let coach = LlmWorkoutCoach::new(
          chat_port.clone(),
          Arc::new(FixedGeminiConfigProvider),
          Arc::new(StubTrainingContextBuilder),
          FixedClock,
      )
      .with_data_port(Arc::new(crate::support::EmptySelectedWorkoutDataPort));

      coach
          .reply("user-1", &sample_summary(), "How did I do?", None)
          .await
          .unwrap();

      let prompt = &chat_port.requests()[0].system_prompt;
      assert!(prompt.contains("if bl, pc, and c5 are not sufficient"));
      assert!(prompt.contains("get_selected_workout"));
  }
  ```

- [ ] **Step 4: Run the focused adapter test file and confirm failure**
  
  Run:
  ```bash
  cargo test --test llm_adapters coaching -- --nocapture
  ```
  
  Expected:
  - new assertions fail because the prompt text does not yet contain the new rules
  - no unrelated failures introduced

---

### Task 3: Harden The Workout Summary Coach Base Prompt

**Files:**
- Modify: `src/adapters/llm/workout_summary_coach.rs`
- Test: `tests/llm_adapters/coaching.rs`

- [ ] **Step 1: Update `WORKOUT_COACH_SYSTEM_PROMPT_BASE` with evidence hierarchy**
  
  Extend the prompt with explicit guidance like this, adapted to the repo’s existing wording and style:

  ```rust
  const WORKOUT_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Use the packed training context as factual background. Be direct, adult, and concise. Do not flatter, hedge, or act like a yes-man. Challenge weak reasoning when the context does not support it. Keep the conversation focused and practical rather than digressive. In your first reply after a workout, ask all follow-up questions you genuinely need at once instead of stretching them across many turns. The athlete should still feel coached, not interrogated. Ask concrete questions about the workout limiter, legs, breathing, fueling, sleep, stress, pain, readiness for the next days, and any plan constraints when relevant. Add other questions only when the workout characteristics clearly justify them. For completed interval workouts, judge execution quality primarily from the packed workout evidence: `bl` describes the intended block structure and targets, `pc` describes the executed power pattern, and `c5` is supporting cadence evidence. Compare `pc` against `bl` first when deciding whether blocks were actually hit. Use `c5` to support conclusions about control, rhythm, and stability, not as the only basis for execution quality. Treat aggregate metrics such as NP, average power, IF, VI, and TSS as secondary context for the whole session, not as sufficient proof that interval blocks were or were not executed correctly. Do not conclude that interval execution was poor only because whole-workout averages were dragged down by recovery valleys, coasting, zeros, terrain, or wind. If the packed evidence is not sufficient for a confident execution judgment, use the available workout tools to inspect higher-fidelity workout data before making a strong claim. If you already have enough information to generate the plan, say that clearly and tell the athlete to save the summary. Return your final answer as JSON only matching the workout summary coach reply schema. The summary may use markdown. Questions may be an empty array when you are ready. Do not output any text outside the JSON object. Do not invent details beyond the provided context.";
  ```

- [ ] **Step 2: Keep the change minimal**
  
  Do not:
  - change `build_stable_context(...)`
  - change `build_volatile_context(...)`
  - change request assembly
  - change tool-loop orchestration

  Only refine prompt semantics.

- [ ] **Step 3: Re-run focused prompt tests**
  
  Run:
  ```bash
  cargo test --test llm_adapters coaching -- --nocapture
  ```
  
  Expected:
  - the three new prompt tests pass
  - existing coaching prompt tests still pass

---

### Task 4: Harden Tool Guidance For Workout Summary Chat

**Files:**
- Modify: `src/domain/llm_tools/mod.rs`
- Possibly modify: `src/domain/llm_tools/get_selected_workout/mod.rs`
- Possibly modify: `src/domain/llm_tools/selected_workout_power_curve/mod.rs`

- [ ] **Step 1: Prefer the smallest correct place for the guidance change**
  
  First inspect whether the cleanest change is:
  - updating the tool-specific `prompt_guidance()` strings in each tool module, or
  - introducing a scope-specific guidance suffix inside `src/domain/llm_tools/mod.rs`

  Prefer the smaller change that avoids changing tool semantics for other scopes unless needed.

- [ ] **Step 2: Adjust `get_selected_workout` guidance**
  
  Update guidance so it explicitly matches this fallback role:

  ```rust
  Some(
      "use for a specific date when packed workout evidence such as bl, pc, and c5 is not sufficient to judge completed-workout execution confidently; prefer this before making a strong execution claim from aggregate metrics alone",
  )
  ```

  If this wording would incorrectly affect other scopes, instead append a `WorkoutSummaryChat`-specific sentence in `tool_prompt_guidance_for_scope(...)`.

- [ ] **Step 3: Adjust `selected_workout_power_curve` guidance**
  
  Update guidance so it does not look like the primary tool for target-compliance judgment:

  ```rust
  Some(
      "use after selecting a completed workout when the answer needs mean-max power or duration-specific power facts; this complements execution analysis but does not replace comparing planned blocks against packed workout evidence or higher-fidelity workout detail",
  )
  ```

- [ ] **Step 4: Preserve generic tool behavior**
  
  Do not:
  - change tool availability rules
  - change tool schemas
  - change execution logic
  - change provider support rules

  This task is guidance-only.

- [ ] **Step 5: Add or update focused tests for guidance text**
  
  If existing tests already assert tool-guidance text in `src/domain/llm_tools/mod.rs`, extend them. Otherwise add a focused test asserting the generated prompt for `WorkoutSummaryChat` mentions:
  - `get_selected_workout`
  - insufficiency of `bl`, `pc`, `c5`
  - not guessing from aggregate metrics
  - `selected_workout_power_curve` as supplemental, not primary for block-hit judgment

- [ ] **Step 6: Run focused tool-guidance tests**
  
  Run:
  ```bash
  cargo test llm_tools:: -- --nocapture
  ```
  
  If that selector is too broad in practice, run the narrowest relevant target that covers the modified guidance assertions.

  Expected:
  - updated guidance tests pass
  - no tool execution behavior changes

---

### Task 5: Run Targeted Verification For The Full Prompt Path

**Files:**
- Read-only verification across:
  - `src/adapters/llm/workout_summary_coach.rs`
  - `src/domain/llm_tools/mod.rs`
  - `tests/llm_adapters/coaching.rs`

- [ ] **Step 1: Run the full adapter test binary**
  
  Run:
  ```bash
  cargo test --test llm_adapters -- --nocapture
  ```
  
  Expected:
  - adapter tests pass
  - no regressions in existing prompt/content assertions

- [ ] **Step 2: Run formatting check**
  
  Run:
  ```bash
  cargo fmt --all --check
  ```
  
  Expected:
  - no formatting issues

- [ ] **Step 3: Run clippy with CI flags**
  
  Run:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  
  Expected:
  - clean pass
  - no lint regressions from prompt-test changes

- [ ] **Step 4: Run architecture verification required by repo**
  
  Run:
  ```bash
  bun run verify:arch
  ```
  
  Expected:
  - pass
  - confirms no architecture-boundary regression from the prompt-only change

---

### Task 6: Review Loop And Regression Challenge

**Files:**
- Review changed files only

- [ ] **Step 1: Strict reviewer pass**
  
  Manually inspect for:
  - any wording that still allows aggregate metrics to overrule `bl`/`pc`/`c5`
  - any wording that accidentally makes tool calls mandatory every time
  - any wording that implies `c5` alone can prove block execution

- [ ] **Step 2: Very strict reviewer pass**
  
  Manually inspect for:
  - accidental spillover of guidance to unrelated coach scopes
  - contradictions between base prompt and tool guidance
  - test wording that is too brittle to minor phrasing edits

- [ ] **Step 3: Nitpicker pass**
  
  Manually inspect for:
  - duplicated phrases in prompt
  - awkward wording like “primary evidence” repeated too many times
  - vague phrases like “better data” instead of “higher-fidelity workout data”

- [ ] **Step 4: Re-run the smallest relevant verification after any review edits**
  
  If review changes only prompt strings:
  ```bash
  cargo test --test llm_adapters coaching -- --nocapture
  ```
  
  If guidance text also changed:
  ```bash
  cargo test --test llm_adapters -- --nocapture
  ```

---

### Task 7: Record The Lesson And Review History

**Files:**
- Modify: `reviewers.md`
- Modify: `tasks/lessons.md`

- [ ] **Step 1: Add a `reviewers.md` entry for this failure mode**
  
  Add a new top entry capturing:
  - Source: user
  - Scope: workout-summary coach prompt
  - Problem: model inferred poor interval execution from whole-session aggregate metrics even though packed context contained stronger execution evidence (`bl`, `pc`, `c5`)
  - Fix: prompt now prioritizes `bl`, `pc`, `c5` for execution judgment and uses tools only when that evidence is insufficient
  - Prevention: before shipping any workout-analysis prompt change, verify the evidence hierarchy explicitly prefers interval/block evidence over whole-session averages

- [ ] **Step 2: Add a reusable lesson to `tasks/lessons.md`**
  
  Add a short reusable lesson along the lines of:
  - for interval workouts, whole-session aggregates are summary context, not proof of block execution quality
  - if packed context contains stronger local execution signals, prompt those first
  - tools should resolve uncertainty, not replace good primary evidence selection

- [ ] **Step 3: Keep both entries concise and reusable**
  
  Do not mention one-off conversational details. Make them reusable for future prompt design and review.

---

### Task 8: Final Repo-Specific Completion Steps

**Files:**
- Regenerated: `graphify-out/**` via script
- Optional plan notes: `docs/superpowers/plans/2026-05-22-workout-summary-coach-prompt-hardening.md`

- [ ] **Step 1: Rebuild graphify after code changes**
  
  Run:
  ```bash
  ./scripts/rebuild_graphify.sh
  ```
  
  Expected:
  - graph rebuild completes successfully
  - `graphify-out/GRAPH_REPORT.md` refreshes

- [ ] **Step 2: Re-run the shortest confidence check after graph rebuild if no code changed during rebuild**
  
  Run:
  ```bash
  cargo test --test llm_adapters coaching -- --nocapture
  ```
  
  Expected:
  - still green

- [ ] **Step 3: Inspect final diff**
  
  Run:
  ```bash
  git diff -- src/adapters/llm/workout_summary_coach.rs src/domain/llm_tools/mod.rs tests/llm_adapters/coaching.rs reviewers.md tasks/lessons.md
  ```
  
  Expected:
  - only prompt/guidance/tests/lessons changes
  - no unrelated edits

---

### Suggested Commit Strategy

Use small commits if implementing incrementally:

1. `test: cover workout summary execution evidence hierarchy`
2. `fix: prioritize packed execution signals in workout coach prompt`
3. `docs: record prompt evidence-hierarchy lesson`

If doing one commit, keep it similarly scoped:
- `fix: harden workout summary coach execution prompt`

---

### Execution Notes

Key implementation guardrails:
- prefer `bl`, `pc`, `c5` as the explicit evidence hierarchy
- keep aggregate metrics secondary by instruction, not removed from context
- do not force tool usage for every reply
- keep the change local and minimal
- keep tests focused on prompt semantics, not model behavior randomness

Key success criteria:
- prompt explicitly defines `bl`/`pc`/`c5` as primary evidence for interval-workout execution quality
- prompt explicitly rejects whole-session aggregates as sufficient proof of missed interval targets
- prompt explicitly allows tools as escalation when packed evidence is insufficient
- existing workout-summary JSON-output contract remains unchanged

### Recommended execution mode

1. Subagent-Driven (recommended): implement task-by-task with reviews between tasks.
2. Inline Execution: execute directly in one session with checkpoints after tests and prompt/guidance changes.
