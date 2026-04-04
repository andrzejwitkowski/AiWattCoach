# Websocket Disconnect Queue Handling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stop processing queued workout-summary websocket messages after the client disconnects, while still allowing the already in-flight reply to finish.

**Architecture:** Replace the current split read-loop plus background worker design in `src/adapters/rest/workout_summary/ws.rs` with a single async control loop that owns both websocket input handling and queued message draining. This keeps disconnect observation and queue start decisions in one place so a buffered close frame can prevent later queued work from starting, without changing queue limits, message ordering, or existing user-facing error payloads.

**Tech Stack:** Rust, Axum websocket support, Tokio `select!`, Tokio `mpsc`, existing workout-summary domain service and REST tests.

---

## Context

Current behavior in `src/adapters/rest/workout_summary/ws.rs`:
- websocket input is read in one loop
- validated user messages are pushed into an in-memory `mpsc` queue
- a background worker task drains that queue and calls `process_send_message(...)`

Current failing verification:
- `cargo test -j 1 -- --nocapture`
- failing test: `tests/workout_summary_rest/ws_endpoints.rs::websocket_disconnect_does_not_generate_queued_follow_up_replies`

Root cause:
- the background worker can start processing the next queued message before the read loop observes a close frame from the client
- this makes disconnect handling timing-dependent across two tasks

Desired rule, approved by user:
- after disconnect, drop queued work that has not started yet
- do not try to cancel the already in-flight reply

Non-goals:
- no durable queue or recovery redesign
- no API shape changes
- no changes to queue capacity or validation rules
- no changes to coach typing/system message payload shapes

---

### Task 1: Lock Down The Failing Disconnect Contract

**Files:**
- Modify: `tests/workout_summary_rest/ws_endpoints.rs`
- Modify: `tests/workout_summary_rest/shared/workout_summary.rs`

**Step 1: Keep the disconnect test focused on the real contract**

Update `websocket_disconnect_does_not_generate_queued_follow_up_replies` so it asserts:
- first queued user message may be fully processed
- second queued user message must never start after disconnect
- persisted summary contains only the first user turn and its coach reply

The test should assert against `service.processed_user_messages()` and final summary contents, not assume the first turn stops midway.

**Step 2: Keep the controllable test delay only in the fake service**

Retain a small configurable delay in `TestWorkoutSummaryService::generate_coach_reply(...)` so the test reliably creates this order:
1. first queued message starts
2. first websocket frame is received by client
3. client closes socket
4. second queued message must not start

**Step 3: Run the test and verify it fails for the current production code**

Run:
```bash
cargo test --test workout_summary_rest websocket_disconnect_does_not_generate_queued_follow_up_replies -- --nocapture
```

Expected:
- FAIL
- `processed_user_messages()` still shows `"First", "Second"`

---

### Task 2: Remove Cross-Task Queue Draining Race

**Files:**
- Modify: `src/adapters/rest/workout_summary/ws.rs`

**Step 1: Remove the background worker task**

Delete the spawned worker that currently drains `queued_messages_rx` in parallel with websocket reads.

Keep:
- `mpsc` queue for bounded buffering
- `MAX_QUEUED_MESSAGES`
- `process_send_message(...)`

Remove only the extra task boundary that allows disconnect races.

**Step 2: Introduce a single owner loop for both socket reads and queued work**

Refactor `handle_socket(...)` so the main loop owns both:
- reading `receiver.next()` for websocket frames
- starting the next queued message only when no send is currently in progress

Recommended structure:
- maintain local state such as `processing_message: bool` or equivalent
- use `tokio::select!` to prioritize whichever event is ready
- only poll `queued_messages_rx.recv()` when it is valid to start a new queued item

Important behavior:
- if a close frame or websocket read error is observed before the next queued item starts, stop the loop and drop remaining queued items
- if `process_send_message(...)` is already running, allow it to finish

**Step 3: Keep current close/error semantics intact**

Preserve existing behavior for:
- invalid websocket payloads
- unsupported websocket message type
- blank message validation
- queue full errors
- `should_close_worker(...)` style close decisions for fatal service errors

The refactor should be behavioral-noop except for the disconnect/queued-work race.

**Step 4: Keep ordering guarantees explicit**

Ensure the refactor still guarantees:
- one queued message is processed at a time
- `coach_typing` precedes `coach_message` for each processed turn
- queued user messages still complete in FIFO order while connected

---

### Task 3: Prove Existing Behavior Still Holds

**Files:**
- Reuse existing tests in `tests/workout_summary_rest/ws_endpoints.rs`
- Reuse existing tests in `tests/llm_rest/workout_summary_flow.rs`

**Step 1: Run the disconnect regression test again**

Run:
```bash
cargo test --test workout_summary_rest websocket_disconnect_does_not_generate_queued_follow_up_replies -- --nocapture
```

Expected:
- PASS

**Step 2: Run the queue-order websocket test**

Run:
```bash
cargo test --test workout_summary_rest websocket_queues_multiple_user_messages_in_order -- --nocapture
```

Expected:
- PASS
- proves FIFO queue semantics still hold

**Step 3: Run the queue-full websocket test**

Run:
```bash
cargo test --test workout_summary_rest websocket_rejects_messages_when_queue_is_full -- --nocapture
```

Expected:
- PASS
- proves bounded queue handling still works after refactor

**Step 4: Run typing/message ordering websocket test**

Run:
```bash
cargo test --test workout_summary_rest websocket_sends_typing_then_coach_message -- --nocapture
```

Expected:
- PASS

**Step 5: Run the athlete-summary websocket coverage that depends on the same path**

Run:
```bash
cargo test --test llm_rest workout_summary_websocket_sends_system_message_before_reply_when_summary_generation_is_needed -- --nocapture
```

Run:
```bash
cargo test --test llm_rest workout_summary_websocket_skips_system_message_when_athlete_summary_is_fresh -- --nocapture
```

Expected:
- both PASS

---

### Task 4: Full Backend Verification In The Low-Memory Windows Mode

**Files:**
- No code changes required

**Step 1: Run serialized backend suite**

Run:
```bash
cargo test -j 1 -- --nocapture
```

Expected:
- PASS

Rationale:
- parallel `cargo test` has already hit Windows pagefile / mmap limits in this environment
- `-j 1` is the current practical full-suite verification mode here

**Step 2: If `cargo test -j 1` passes, re-run the standard backend checks if touched files require it**

Run:
```bash
cargo fmt --all --check
```

Run:
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:
- both PASS

Only fix issues directly caused by this websocket refactor.

---

### Task 5: Clean Up Test Helper Scope

**Files:**
- Modify: `tests/workout_summary_rest/shared/workout_summary.rs`

**Step 1: Keep the helper minimal**

If the temporary fake-service delay helper remains useful after the refactor, keep it narrowly named and scoped to websocket timing tests.

If the disconnect test no longer needs it once the single-loop refactor lands, delete it.

Goal:
- no extra test helper complexity beyond what the final test suite needs

**Step 2: Re-run the websocket rest suite after any cleanup**

Run:
```bash
cargo test --test workout_summary_rest -- --nocapture
```

Expected:
- PASS

---

## Implementation Notes

- Keep the diff minimal. Do not redesign the workout-summary domain service.
- Do not change REST DTOs or websocket payload schemas.
- Do not introduce cancellation of in-flight domain/LLM work; only stop later queued messages from starting.
- Prefer one clearly structured loop in `handle_socket(...)` over adding more flags, notifiers, or nested channels.
- Preserve the thin-adapter rule: websocket code should orchestrate transport behavior, not absorb business logic.

## Risks To Watch

- accidentally starving websocket reads while waiting on queue processing
- accidentally allowing concurrent `process_send_message(...)` tasks
- changing close behavior for fatal service errors
- making tests pass by over-fitting timing rather than removing the race

## Definition Of Done

- disconnect test proves the second queued message does not start after client close
- queue ordering, queue full, and typing/message order tests still pass
- athlete-summary websocket tests still pass
- `cargo test -j 1 -- --nocapture` passes in this environment
- any extra fake-service timing helper is either justified or removed
