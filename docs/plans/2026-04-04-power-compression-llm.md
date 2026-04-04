# Power Compression For LLM Context Implementation Plan

**Goal:** Replace LLM workout power arrays with compressed raw-power segments and describe the encoding in the workout coach prompt.

**Architecture:** Keep compression in `domain/training_context`, where workout stream shaping already exists, and keep the LLM adapter limited to prompt assembly. Do not change persistence, REST DTOs, or unrelated consumers.

**Tech Stack:** Rust, serde, existing training-context packing, existing LLM workout coach adapter

---

### Task 1: Save the approved data shape

**Files:**
- Modify: `src/domain/training_context/model.rs`

**Step 1: Write the failing tests**

Use the downstream packing and service tests in later tasks to fail until the field exists.

**Step 2: Write minimal implementation**

Replace the recent workout power field with compressed power segments:

```rust
pub compressed_power_levels: Vec<String>,
```

**Step 3: Re-run downstream tests**

Run the packing and service tests after they are added.

### Task 2: Add compression tests and implementation

**Files:**
- Modify: `src/domain/training_context/service.rs`

**Step 1: Write the failing tests**

Add focused tests for:

- steady power encodes into one run
- 1-second non-FTP spike under 3 levels is smoothed
- 1-second FTP-zone change is preserved
- missing FTP yields no compressed output

**Step 2: Run tests to verify failure**

Run:

```bash
cargo test compressed_power -- --nocapture
```

Expected: FAIL for the new behavior.

**Step 3: Write minimal implementation**

- extract raw numeric values from the `watts` stream
- encode levels using the provided formula
- smooth eligible 1-second spikes/dips
- RLE the encoded levels into `Vec<String>`
- wire compressed output into `build_recent_workout()` using `activity.metrics.ftp_watts`

**Step 4: Re-run targeted tests**

Run:

```bash
cargo test compressed_power -- --nocapture
```

Expected: PASS.

### Task 3: Pack `pc` instead of `p5`

**Files:**
- Modify: `src/domain/training_context/packing.rs`

**Step 1: Write the failing test**

Update packing tests to expect `"pc":[...]` and not `"p5":[...]`.

**Step 2: Run test to verify failure**

Run:

```bash
cargo test compact_render -- --nocapture
```

Expected: FAIL.

**Step 3: Write minimal implementation**

Map `RecentWorkoutContext.compressed_power_levels` to compact field `pc` and keep omission-on-empty behavior.

**Step 4: Re-run targeted test**

Run:

```bash
cargo test compact_render -- --nocapture
```

Expected: PASS.

### Task 4: Update the workout coach prompt

**Files:**
- Modify: `src/adapters/llm/workout_summary_coach.rs`
- Modify: `tests/llm_adapters.rs`

**Step 1: Write the failing test**

Assert the system prompt mentions:

- `pc`
- `"level:seconds"`
- `round((watts / ftp)^2.5 * 100)`
- the 1-second smoothing rule with the FTP-zone exception

**Step 2: Run test to verify failure**

Run:

```bash
cargo test llm_workout_coach --test llm_adapters -- --nocapture
```

Expected: FAIL.

**Step 3: Write minimal implementation**

Extend `WORKOUT_COACH_SYSTEM_PROMPT` with a concise description of the compression function.

**Step 4: Re-run targeted test**

Run:

```bash
cargo test llm_workout_coach --test llm_adapters -- --nocapture
```

Expected: PASS.

### Task 5: Update LLM flow assertions

**Files:**
- Modify: `tests/llm_rest/workout_summary_flow.rs`

**Step 1: Write the failing test**

Assert the live request still includes `training_context_volatile=` and, when fixture data includes streams and FTP, includes `"pc"` rather than `"p5"`.

**Step 2: Run test to verify failure**

Run:

```bash
cargo test send_message_uses_saved_openrouter_settings_through_live_adapter --test llm_rest -- --nocapture
```

Expected: FAIL.

**Step 3: Write minimal implementation or fixture updates**

Adjust test fixture data only as needed so the request exposes the compressed field.

**Step 4: Re-run targeted test**

Run:

```bash
cargo test send_message_uses_saved_openrouter_settings_through_live_adapter --test llm_rest -- --nocapture
```

Expected: PASS.

### Task 6: Run full verification

**Files:**
- No file changes expected

**Step 1: Run formatting**

```bash
cargo fmt --all --check
```

**Step 2: Run linting**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run Rust tests**

```bash
cargo test
```

**Step 4: Run repo verification gate**

```bash
bun run test:all
```

Expected: all pass, with output reviewed before claiming completion.
