# React Shell Design

## Goal

Add a separate frontend application for AiWattCoach using Bun, Vite, React, and Tailwind, with a minimal application shell, a dedicated settings entry point, and a real connection to the Rust backend.

## Recommended Approach

Keep the frontend isolated in `frontend/` so the existing Rust backend stays API-focused. Use a thin client-side API layer that talks to the backend over explicit HTTP endpoints, starting with `GET /health` and `GET /ready`.

## Structure

- `frontend/src/app/*` owns shell composition and layout.
- `frontend/src/pages/*` owns route-level screens.
- `frontend/src/lib/api/*` owns HTTP calls and DTO mapping.
- `frontend/src/config/*` owns environment-driven frontend configuration.
- Backend remains responsible for service truth and health semantics.

## First Delivered Behavior

- The frontend boots with a responsive application shell.
- The shell exposes a navigation entry point to settings.
- The app calls the backend on startup and shows connection state.
- The settings screen lets the user inspect API configuration and re-check backend readiness.

## Integration Direction

- Development uses a frontend-owned API base URL with a Vite proxy fallback.
- The frontend talks to the backend directly over logical endpoints, without mocks in runtime flows.
- Elysia is optional for future BFF/proxy work, but not required for the first delivery.

## Testing Strategy

- Start with failing tests for shell rendering and connection-state behavior.
- Add a failing test for the API client against mocked fetch responses.
- Implement the minimum UI and client code to make those tests pass.
- Run frontend tests, frontend build, and backend tests before calling the task done.
