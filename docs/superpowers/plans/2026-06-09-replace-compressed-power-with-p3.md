# Replace Compressed `pc` With True 3-Second Power (`p3`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop sending run-length-encoded compressed power (`pc`) to LLM coaches and tool fallbacks; send true average watts in 3-second buckets (`p3`) in packed context and `get_selected_workout*` tool responses; update all prompt/legend/tool-guidance text to say this is real power data, not compressed encoding.

**Architecture:** Extract shared 3-second watt bucketing into a small `domain/workout_streams` module used by both `training_context` (packed `p3`) and `llm_tools` (`get_selected_workout` / `get_selected_workout_by_id` tool responses). Cadence stays on 5-second buckets as `c5`. Rename the packed field from `pc` (`Vec<String>`) to `p3` (`Vec<i32>`). Delete the FTP-encoded RLE compressor (`compress_power_stream` and helpers). Update tool prompt guidance and tool response shaping so escalation paths also return true 3s watts — not compressed `pc`, not sparse 1s index sampling.

**Tech Stack:** Rust, serde, existing training-context packing, workout-summary / training-plan / meso-cycle LLM prompts

**Branch note:** `origin/main` is at `d7fe265` (deslop after meso merge). Sync before implementing:

```bash
git fetch origin main
git merge origin/main   # or rebase if branch policy prefers it
```

---

## Current State (why change)

| Layer | Today |
|-------|-------|
| Domain field | `compressed_power_levels: Vec<String>` on `RecentWorkoutContext` and `HistoricalWorkoutContext` |
| Packed JSON key | `pc` — array of `"level:seconds"` strings |
| Algorithm | 1s watts → 10W bucket → FTP-relative level → spike smooth → RLE → optional run-cap to 48 chunks |
| FTP required | Yes — empty `pc` when FTP missing/invalid |
| Prompt legend | `PACKED_TRAINING_CONTEXT_LEGEND` explains compressed encoding |
| Workout coach prompt | References `pc as executed power pattern` alongside `bl` and `c5` |
| Tool prompt guidance | `get_selected_workout` fallback text cites `bl, pc, and c5` |
| Tool responses | `get_selected_workout*` returns sparse-sampled 1s `watts` stream (256 points) — not compressed, but inconsistent with `p3` |

Cadence already uses the desired pattern: `extract_and_average_stream(..., "cadence")` with `STREAM_BUCKET_SIZE = 5` → packed as `c5`.

---

## Target State

| Layer | Target |
|-------|--------|
| Domain field | `power_values_3s: Vec<i32>` |
| Packed JSON key | `p3` — array of integer watts (3-second mean per bucket) |
| Algorithm | 1s watts → 3s average → `compress_stream_chunks` cap at 48 values (same token guard as today) |
| FTP required | No — raw watts only; omit `p3` when watts stream empty |
| Prompt legend | `p3` = true average watts in 3-second buckets; explicitly **not** compressed / encoded |
| Workout coach prompt | `p3` as primary executed-power evidence (replace `pc` references) |
| Tool prompt guidance | `bl, p3, and c5`; tools are fallback when packed true-power evidence is insufficient |
| Tool responses | `watts` stream in `get_selected_workout*` = 3s average watts array (true values), capped at 256 buckets |

**Unchanged:** `selected_workout_power_curve` — mean-max duration curve, not compressed workout power. Keep as supplemental duration-specific tool.

**Fixture expectation** (`ride-1` watts `[200,220,240,260,280]`):
- 3s buckets: `[(200+220+240)/3, (260+280)/2]` → `[220, 270]`
- Packed: `"p3":[220,270]` (not `"pc":["36:1",...]`)

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/domain/workout_streams/mod.rs` | Shared `average_into_buckets`, `cap_stream_buckets`, constants |
| Modify | `src/domain/mod.rs` | `pub mod workout_streams` |
| Modify | `src/domain/training_context/service/mod.rs` | `MAX_CHUNKS_PER_WORKOUT`; delegate bucketing to `workout_streams` |
| Modify | `src/domain/training_context/service/power.rs` | Thin wrapper over `workout_streams`; remove `compress_power_stream` and RLE helpers |
| Modify | `src/domain/llm_tools/get_selected_workout/response.rs` | Bucket `watts` to 3s means; other streams keep index sampling |
| Modify | `src/domain/llm_tools/get_selected_workout/mod.rs` | Tool description: true 3s average watts |
| Modify | `src/domain/llm_tools/mod.rs` | `scope_specific_tool_guidance`: `p3` not `pc` |
| Modify | `src/domain/llm_tools/get_selected_workout/tests.rs` | 3s watt bucket assertions |
| Modify | `src/domain/training_context/service/context.rs` | Build `power_values_3s` instead of `compressed_power_levels` |
| Modify | `src/domain/training_context/model.rs` | Rename fields on recent + historical workout contexts |
| Modify | `src/domain/training_context/packing/payloads/volatile.rs` | `pc` → `p3: &'a [i32]` |
| Modify | `src/domain/training_context/packing/payloads/stable.rs` | `pc` → `p3: &'a [i32]` |
| Modify | `src/domain/llm/context_prelude.rs` | Update `PACKED_TRAINING_CONTEXT_LEGEND` |
| Modify | `src/domain/workout_summary/prompt.rs` | Evidence hierarchy: `bl`, `p3`, `c5`; true-power wording |
| Modify | `frontend/src/features/admin-prompt-preview/utils/decodePackedContext.ts` | `p3: 'Power 3s (watts)'`; remove misleading `pc` label |
| Modify | `src/domain/training_context/service/tests/power.rs` | Replace compressed tests with 3s-bucket tests |
| Modify | `src/domain/training_context/service/tests/builder/*.rs` | Update field + JSON assertions |
| Modify | `src/domain/training_context/packing/tests.rs` | `"p3":[...]` assertions |
| Modify | `tests/llm_rest/workout_summary_flow.rs` | `"p3"` not `"pc"` |
| Modify | `tests/llm_adapters/coaching.rs` | Prompt asserts `p3` |
| Modify | `tests/llm_adapters/training_plan.rs` | Legend asserts `p3` |
| Modify | `src/adapters/llm/workout_summary_coach.rs` | Unit test prompt fragments |
| Delete logic | `compress_power_stream` + encode/smooth/RLE helpers in `power.rs` | Dead after migration |

---

### Task 1: Sync branch, shared bucketing module, failing tests

**Files:**
- Create: `src/domain/workout_streams/mod.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/training_context/service/power.rs`
- Modify: `src/domain/training_context/service/tests/power.rs`

- [ ] **Step 1: Merge `origin/main`**

```bash
git fetch origin main && git merge origin/main
```

- [ ] **Step 2: Add shared bucketing module**

```rust
// src/domain/workout_streams/mod.rs
pub const POWER_BUCKET_SECONDS: usize = 3;
pub const CADENCE_BUCKET_SECONDS: usize = 5;

pub fn average_into_buckets(values: &[i32], bucket_size: usize) -> Vec<i32> {
    values
        .chunks(bucket_size)
        .map(|chunk| (chunk.iter().sum::<i32>() as f64 / chunk.len() as f64).round() as i32)
        .collect()
}

pub fn cap_stream_buckets(buckets: Vec<i32>, max_buckets: usize) -> Vec<i32> {
    if max_buckets == 0 || buckets.len() <= max_buckets {
        return buckets;
    }
    let recent_count = max_buckets / 2;
    let summary_count = max_buckets - recent_count;
    let older_count = buckets.len() - recent_count;
    let group_size = older_count.div_ceil(summary_count);
    let summarized = buckets[..older_count]
        .chunks(group_size)
        .map(|group| (group.iter().sum::<i32>() as f64 / group.len() as f64).round() as i32);
    summarized
        .chain(buckets[older_count..].iter().copied())
        .collect()
}

pub fn bucket_and_cap(values: &[i32], bucket_size: usize, max_buckets: usize) -> Vec<i32> {
    cap_stream_buckets(average_into_buckets(values, bucket_size), max_buckets)
}
```

Move logic from `compress_stream_chunks` in `power.rs` into `cap_stream_buckets` (delete duplicate).

- [ ] **Step 3: Thin `power.rs` wrapper**

```rust
pub(super) fn extract_and_average_stream(
    streams: &[ActivityStream],
    stream_type: &str,
    bucket_size: usize,
) -> Vec<i32> {
    let values = extract_raw_stream(streams, stream_type);
    workout_streams::bucket_and_cap(&values, bucket_size, super::MAX_CHUNKS_PER_WORKOUT)
}

pub(super) fn extract_power_values_3s(streams: &[ActivityStream]) -> Vec<i32> {
    extract_and_average_stream(streams, "watts", workout_streams::POWER_BUCKET_SECONDS)
}
```

Update cadence call site to pass `CADENCE_BUCKET_SECONDS` (5).

- [ ] **Step 4: Write failing tests** (replace `compressed_power_*` tests)

```rust
#[test]
fn power_3s_averages_watts_into_three_second_buckets() {
    assert_eq!(
        extract_and_average_stream(
            &watts_stream(&[200, 220, 240, 260, 280]),
            "watts",
            3,
        ),
        vec![220, 270],
    );
}

#[test]
fn power_3s_returns_empty_without_watts_stream() {
    assert!(extract_and_average_stream(&[], "watts", 3).is_empty());
}

#[test]
fn power_3s_applies_chunk_cap_for_long_streams() {
    let noisy: Vec<i32> = (0..(MAX_CHUNKS_PER_WORKOUT * 6 + 1))
        .map(|i| if i % 2 == 0 { 300 } else { 0 })
        .collect();
    assert!(
        extract_and_average_stream(&watts_stream(&noisy), "watts", 3).len()
            <= MAX_CHUNKS_PER_WORKOUT
    );
}

#[test]
fn power_3s_preserves_missing_samples_as_zero_in_bucket_average() {
    let streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: None,
        data: Some(serde_json::json!([200, null, 210])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];
    assert_eq!(extract_and_average_stream(&streams, "watts", 3), vec![137]);
}
```

Add a small `watts_stream` test helper mirroring existing cadence fixture style.

- [ ] **Step 5: Run tests — expect FAIL** (old tests still reference `compress_power_stream`)

```bash
CARGO_BUILD_JOBS=1 cargo test power_3s -- --nocapture
```

---

### Task 2: Domain model + context builder

**Files:**
- Modify: `src/domain/training_context/model.rs`
- Modify: `src/domain/training_context/service/context.rs`

- [ ] **Step 1: Rename fields**

```rust
// RecentWorkoutContext + HistoricalWorkoutContext
pub power_values_3s: Vec<i32>,
```

- [ ] **Step 2: Wire builder**

In `build_recent_workout`:

```rust
let power_values_3s = extract_power_values_3s(&activity.details.streams);
```

In `build_historical_context` workout map — same, no FTP gate:

```rust
let power_values_3s = workout_sources
    .detailed_activities_by_id
    .get(&activity.id)
    .map(|detailed| extract_power_values_3s(&detailed.details.streams))
    .unwrap_or_default();
```

Remove `compress_power_stream` / `extract_power_stream` imports from `context.rs`.

- [ ] **Step 3: Fix compile errors** in tests/fixtures referencing `compressed_power_levels`.

---

### Task 3: Packing `p3` instead of `pc`

**Files:**
- Modify: `src/domain/training_context/packing/payloads/volatile.rs`
- Modify: `src/domain/training_context/packing/payloads/stable.rs`
- Modify: `src/domain/training_context/packing/tests.rs`
- Modify: `src/domain/training_context/service/tests/builder/rendering.rs`
- Modify: `src/domain/training_context/service/tests/builder/focus_and_aliases.rs`

- [ ] **Step 1: Update compact structs**

```rust
#[serde(skip_serializing_if = "is_empty_slice")]
p3: &'a [i32],
// remove pc: &'a [String]
```

Map from `workout.power_values_3s`.

- [ ] **Step 2: Update packing tests**

```rust
power_values_3s: vec![220, 270],
// ...
assert!(rendered.volatile_context.contains("\"p3\":[220,270]"));
assert!(!rendered.volatile_context.contains("\"pc\":"));
```

- [ ] **Step 3: Update rendering test** for `ride-1` fixture:

```rust
assert_eq!(recent_day.workouts[0].power_values_3s, vec![220, 270]);
assert!(result.rendered.volatile_context.contains("\"p3\":[220,270]"));
```

- [ ] **Step 4: Run targeted tests**

```bash
CARGO_BUILD_JOBS=1 cargo test compact_render -- --nocapture
CARGO_BUILD_JOBS=1 cargo test training_context::service::tests::builder::rendering -- --nocapture
```

---

### Task 4: Remove compressed-power implementation

**Files:**
- Modify: `src/domain/training_context/service/power.rs`

- [ ] **Step 1: Delete** `compress_power_stream`, `encode_power_level`, `round_to_nearest_power_bucket`, `smooth_single_second_level_noise`, `run_length_encode_levels`, `EncodedPowerRun`, `compress_encoded_runs`, `parse_encoded_runs`, `summarize_encoded_run_group`, `merge_adjacent_runs`, `format_encoded_runs`, `extract_power_values` (if unused), `POWER_BUCKET_WATTS`.

- [ ] **Step 2: Keep** `extract_raw_stream`, `extract_numeric_values`, parameterized `extract_and_average_stream`, `extract_power_values_3s` (delegating to `workout_streams`).

- [ ] **Step 3: Run power + builder tests**

```bash
CARGO_BUILD_JOBS=1 cargo test power_3s -- --nocapture
CARGO_BUILD_JOBS=1 cargo test training_context::service::tests -- --test-threads=1
```

---

### Task 5: Prompt + legend — true power, not compressed

**Files:**
- Modify: `src/domain/llm/context_prelude.rs`
- Modify: `src/domain/workout_summary/prompt.rs`
- Modify: `src/adapters/llm/workout_summary_coach.rs` (prompt unit tests)
- Modify: `tests/llm_adapters/coaching.rs`
- Modify: `tests/llm_adapters/training_plan.rs`

- [ ] **Step 1: Replace legend tail** in `PACKED_TRAINING_CONTEXT_LEGEND`

Remove the entire `pc` compressed-encoding paragraph. Append:

```
In training_context_volatile and historical workout entries, p3 is an array of true average watts in 3-second buckets from the raw 1-second watts stream. This is not compressed, run-length-encoded, or FTP-normalized power; each integer is watts. Consecutive 1-second samples are averaged within each 3-second bucket; partial trailing buckets use the available samples. Long workouts may cap p3 length by averaging older buckets while preserving recent buckets in full detail, similar to c5.
```

Also add `p3=power watts in 3-second buckets` to the common-inner-fields list; remove `pc` references.

- [ ] **Step 2: Update workout coach system prompt**

In `WORKOUT_COACH_SYSTEM_PROMPT_BASE`, replace:

```
bl as intended block structure/targets, pc as executed power pattern, and c5 as supporting cadence evidence
```

with:

```
bl as intended block structure/targets, p3 as true executed power in 3-second average watts (not compressed or FTP-encoded), and c5 as supporting cadence evidence
```

Keep aggregate-metrics-secondary guidance unchanged.

- [ ] **Step 3: Update adapter tests**

Rename `llm_workout_coach_describes_power_compression_in_system_prompt` → `llm_workout_coach_describes_true_p3_power_in_system_prompt`.

```rust
assert!(prompt.contains("p3 as true executed power"));
assert!(prompt.contains("not compressed"));
assert!(prompt.contains("p3=")); // in legend
assert!(!prompt.contains("pc as executed"));
assert!(!prompt.contains("level:seconds"));
assert!(!prompt.contains("round((watts / ftp)^2.5 * 100)"));
```

In `tests/llm_adapters/training_plan.rs`, replace `pc` / `level:seconds` asserts with `p3` / true-watts wording.

- [ ] **Step 4: Run LLM adapter tests**

```bash
CARGO_BUILD_JOBS=1 cargo test --test llm_adapters coaching -- --nocapture
CARGO_BUILD_JOBS=1 cargo test workout_summary_coach -- --nocapture
```

---

### Task 6: LLM tool guidance + `get_selected_workout` 3s watts

**Files:**
- Modify: `src/domain/llm_tools/mod.rs`
- Modify: `src/domain/llm_tools/get_selected_workout/response.rs`
- Modify: `src/domain/llm_tools/get_selected_workout/mod.rs`
- Modify: `src/domain/llm_tools/get_selected_workout/tests.rs`

- [ ] **Step 1: Update scope-specific tool guidance**

In `scope_specific_tool_guidance`:

```rust
"- For workout-summary execution judgments, `{selected_workout_tool_name}` is the fallback when packed evidence like bl, p3, and c5 is insufficient for a confident call. Tool responses return true watts in 3-second averages for the watts stream — not compressed or FTP-encoded power."
```

Update `workout_summary_prompt_guidance_prioritizes_selected_workout_over_power_curve_for_execution_judgment` test:

```rust
assert!(selected_workout_line.contains("bl, p3, and c5"));
assert!(selected_workout_line.contains("true watts"));
assert!(!selected_workout_line.contains("bl, pc, and c5"));
```

- [ ] **Step 2: Bucket watts in tool response**

In `get_selected_workout/response.rs`, branch `series_values` for watts:

```rust
const MAX_TOOL_WATTS_BUCKETS: usize = 256;

fn stream_data_for_tool(
    stream_type: &str,
    series: Option<&CompletedWorkoutSeries>,
) -> Vec<serde_json::Value> {
    match series {
        Some(CompletedWorkoutSeries::Integers(v))
            if stream_type.eq_ignore_ascii_case("watts") =>
        {
            workout_streams::bucket_and_cap(v, POWER_BUCKET_SECONDS, MAX_TOOL_WATTS_BUCKETS)
                .into_iter()
                .map(|w| json!(w))
                .collect()
        }
        Some(CompletedWorkoutSeries::Integers(v)) => sampled_values(v, MAX_STREAM_SAMPLES),
        // ... existing float/bool/string arms unchanged
        None => Vec::new(),
    }
}
```

Use in `map_completed_workout` instead of calling `series_values` directly for `data`.

- [ ] **Step 3: Update tool description**

```rust
description: "...Returns completed workouts with full statistics, watts as true 3-second average power buckets, other streams as capped raw samples, and AI conversation history..."
```

Optional `prompt_guidance` tweak:

```rust
"use when packed p3/c5/bl evidence is insufficient; watts in the response are true 3-second average watts"
```

- [ ] **Step 4: Update tool tests**

Replace `get_selected_workout_downsamples_large_streams` expectations: 1000×1s zeros/ones → ≤256 **3s buckets** (not 256 sparse 1s indices).

Add focused test:

```rust
#[test]
fn get_selected_workout_buckets_watts_to_three_second_averages() {
    // watts [200,220,240,260,280] → [220,270]
    assert_eq!(stream_data, json!([220, 270]));
}
```

Keep `get_selected_workout_downsampling_keeps_both_ends` for **non-watts** streams only (or split test).

- [ ] **Step 5: Run tool tests**

```bash
CARGO_BUILD_JOBS=1 cargo test get_selected_workout -- --nocapture
CARGO_BUILD_JOBS=1 cargo test llm_tools::tests -- --nocapture
```

---

### Task 7: Frontend admin decode + integration test

**Files:**
- Modify: `frontend/src/features/admin-prompt-preview/utils/decodePackedContext.ts`
- Modify: `tests/llm_rest/workout_summary_flow.rs`

- [ ] **Step 1: Admin label**

```typescript
p3: 'Power 3s (watts)',
// remove pc: 'Power Curve',
```

- [ ] **Step 2: Integration assertion**

```rust
assert!(message_contains("\"p3\":"));
```

- [ ] **Step 3: Run checks**

```bash
CARGO_BUILD_JOBS=1 cargo test --test llm_rest workout_summary_flow -- --nocapture
bun run --cwd frontend test src/features/admin-prompt-preview
```

---

### Task 8: Final verification

- [ ] **Step 1: Format + arch + clippy**

```bash
cargo fmt --all
bun run verify:arch
CARGO_BUILD_JOBS=1 cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] **Step 2: Targeted test sweep**

```bash
CARGO_BUILD_JOBS=1 cargo test power_3s compact_render get_selected_workout training_context::service::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test llm_tools::tests -- --nocapture
CARGO_BUILD_JOBS=1 cargo test --test llm_adapters -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --test llm_rest -- --test-threads=1
```

- [ ] **Step 3: Rebuild graphify** (code files changed)

```bash
./scripts/rebuild_graphify.sh
```

---

## Token / Size Notes

- 3s buckets are ~1.67× denser than old 5s `p5` but far more readable than RLE `pc`.
- Packed `p3`: `MAX_CHUNKS_PER_WORKOUT = 48` via `workout_streams::cap_stream_buckets` — same cap as `c5`.
- Tool `watts`: up to 256 three-second buckets (~12.8 min full-detail tail + summarized older) — on-demand only.
- A 2h ride: ~2400 raw 3s buckets → capped to 48 integers in prompt (~100–150 tokens/workout).
- Removing FTP dependency means power evidence appears even when activity FTP is missing (improvement).

## Risks

| Risk | Mitigation |
|------|------------|
| LLM prompts/cache reference old `pc` key | Coordinated rename to `p3` + legend update in same PR |
| Slightly larger prompts vs RLE | 48-chunk cap unchanged; monitor `approximate_tokens` in tests |
| Historical docs mention `pc` | Optional follow-up: update `docs/plans/2026-04-04-power-compression-llm*.md` with superseded note (not required for behavior) |

## Self-Review (spec coverage)

- [x] Replace compressed packed data with 3s power array → Tasks 2–4
- [x] Prompt says true power, not compressed → Task 5
- [x] Tool guidance + tool responses use `p3`/true 3s watts, not `pc` → Task 6
- [x] Sync remote main → Task 1
- [x] Shared bucketing module for training_context + llm_tools → Task 1
- [x] Tests for builder, packing, prompts, tools, integration → Tasks 1–8
- [x] No placeholder steps
