# Power Compression For LLM Context Design

**Goal:** Replace LLM workout power arrays with compressed raw-power segments that preserve meaningful effort structure while lowering prompt size.

## Scope

- Replace only the LLM-facing recent workout power payload currently packed as `p5`.
- Compute compression from raw 1-second `watts` stream data.
- Use per-activity FTP from `activity.metrics.ftp_watts`.
- Serialize compressed output as an array of `"Level:Seconds"` strings.
- Add a short description of the encoding to the workout coach system prompt.

## Chosen Approach

Implement compression inside `src/domain/training_context`, where workout stream shaping already happens today.

- `build_recent_workout()` will derive a compressed raw-power representation from the activity `watts` stream.
- `RecentWorkoutContext` will store the compressed data instead of the existing 5-second power array.
- `render_training_context()` will emit the compressed data under a new compact field, `pc`.
- `LlmWorkoutCoach` will keep the same stable/volatile context flow, but its system prompt will explain how to interpret `pc`.

## Compression Algorithm

For each raw 1-second power sample `P`, compute encoded level `L` as:

`L = round((P / FTP)^2.5 * 100)`

Then:

- smooth a 1-second spike or dip when the changed level differs from the surrounding level by less than `3`
- do not smooth that 1-second change when the surrounding level is in the FTP zone `90..=110`
- run-length encode consecutive identical levels into `"Level:Seconds"`

Example:

- raw levels: `100,100,100,100,100`
- packed output: `"100:5"`

## Why This Approach

- Smallest correct change: only the LLM-specific packed training context changes.
- Preserves domain boundaries: stream interpretation stays in `training_context`, not the LLM adapter.
- Matches the requested algorithm semantics by using raw 1-second samples rather than existing 5-second buckets.
- Avoids sending both old and new formats, which would increase token usage.

## Data Contract

Recent packed workout data in `training_context_volatile` will use:

- `pc`: compressed power segments as `string[]`

Cadence remains unchanged as `c5` because only power compression is requested.

If the activity lacks valid FTP or lacks raw `watts` data, omit `pc` by emitting an empty list so the compact serializer skips the field.

## Testing

- Add unit tests for raw-power compression behavior in `src/domain/training_context/service.rs`.
- Update packing tests in `src/domain/training_context/packing.rs` to expect `pc` instead of `p5`.
- Update `tests/llm_adapters.rs` to assert the system prompt describes the compression format.
- Update `tests/llm_rest/workout_summary_flow.rs` to assert the live LLM request still carries the packed training context with the new compressed field.
