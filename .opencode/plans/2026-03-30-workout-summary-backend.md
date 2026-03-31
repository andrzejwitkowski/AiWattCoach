# Workout Summary Backend

## Feature Summary

Per-workout summary with RPE rating and AI coach conversation. MongoDB-backed, exposed through REST and WebSocket APIs. The coach is mocked for now; real LLM integration comes later.

## Data Model

- `WorkoutSummary`: `id`, `user_id`, `event_id` (`String`), `rpe` (`Option<u8>` constrained to 1-10), `messages` (`Vec<ConversationMessage>`), `created_at`, `updated_at`
- `ConversationMessage`: `id` (UUID), `role` (`User` or `Coach`), `content`, `created_at`
- Mongo unique index on `(user_id, event_id)` so each user has one summary per workout
- Mongo duplicate-key error code `11000` maps to a domain `AlreadyExists` error

## API Endpoints

| Method | Path | Behavior |
| --- | --- | --- |
| `GET` | `/api/workout-summaries/{event_id}` | Returns `404` if not found |
| `POST` | `/api/workout-summaries/{event_id}` | Creates summary, idempotently returns existing summary if already present |
| `PATCH` | `/api/workout-summaries/{event_id}/rpe` | Updates the workout RPE |
| `POST` | `/api/workout-summaries/{event_id}/messages` | Appends user message and returns persisted coach reply |
| `GET` | `/api/workout-summaries?eventIds=1,2,3` | Batch fetch for sidebar display |
| `GET` | `/api/workout-summaries/{event_id}/ws` | WebSocket chat with typing indicators |

## WebSocket Protocol

- Auth uses the existing session cookie during the upgrade request
- Client message: `{ "type": "send_message", "content": "..." }`
- Server messages:
  - `{ "type": "coach_typing" }`
  - after 1.5s simulated delay: `{ "type": "coach_message", "message": {...} }`
  - `{ "type": "error", "error": "..." }` for validation or processing failures

## Key Design Decisions

- `event_id` is a `String` to avoid coupling the feature to Intervals.icu numeric IDs
- `GET` does not auto-create summaries; missing summary is a real `404`
- `POST` is idempotent and returns the existing summary on duplicate create attempts
- Both REST and WebSocket message flows persist the user message before generating and persisting the mock coach reply
- Handlers stay thin; domain service owns validation and orchestration

## Implementation Tasks

1. Enable Axum `ws` support in `Cargo.toml` and add `tokio-tungstenite` as a dev-dependency for WebSocket integration tests.
2. Introduce domain model files for `WorkoutSummary`, `ConversationMessage`, `MessageRole`, domain errors, and RPE validation.
3. Define the `WorkoutSummaryRepository` port using the existing boxed-future pattern.
4. Create a mock coach module that generates template responses based on the latest user message and optional RPE.
5. Implement `WorkoutSummaryUseCases` and `WorkoutSummaryService` to orchestrate create, fetch, batch fetch, RPE updates, and chat message persistence.
6. Wire the new domain module through `src/domain/workout_summary/mod.rs` and `src/domain/mod.rs`.
7. Implement MongoDB persistence in `src/adapters/mongo/workout_summary.rs` with explicit document mapping and `ensure_indexes()` support.
8. Define REST DTOs for summary payloads, mutation requests, query parsing, and WebSocket message envelopes.
9. Map REST error responses from domain errors to HTTP responses.
10. Translate domain models into DTOs with REST mapping helpers.
11. Implement REST handlers for get, create, update RPE, send message, and batch list.
12. Implement WebSocket handling for upgrade, authentication, client message parsing, typing notifications, simulated delay, and persisted mock coach replies.
13. Register the REST module wiring and new routes in `src/adapters/rest/mod.rs`.
14. Add `workout_summary_service` to `AppState` and wire the repository/service in `main.rs`.
15. Create integration test helpers and fake services matching the repo's current test harness structure.
16. Test REST integration for authentication, create/get, batch fetch, RPE update, and message sending.
17. Verify WebSocket integration for authentication, typing indicator flow, persisted replies, and invalid message handling.
18. Run final verification with formatting, clippy, targeted tests, and full backend test coverage relevant to the feature.
