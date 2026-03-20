# Backend Setup Design

## Goal

Initialize the Rust backend for AiWattCoach with a minimal, testable Axum application, MongoDB connectivity wiring, environment-based configuration, and a project structure that follows the project handbook's hexagonal architecture rules.

## Recommended Approach

Use a single Rust crate for now and organize source code by architectural boundary rather than by framework concerns. Keep the first slice deliberately small: config loading, app state, Mongo client bootstrap, and a thin REST adapter with a health endpoint. This keeps `#3` focused on foundation work while leaving clear expansion points for Intervals, AI, and workout planning.

## Structure

- `src/main.rs` boots config, Mongo connectivity, and the HTTP server.
- `src/config/*` owns settings parsing, app state, and application wiring.
- `src/adapters/rest/*` owns HTTP routes and response mapping only.
- `src/adapters/mongo/*` owns Mongo client creation and persistence-specific setup.
- `src/domain/system/*` holds minimal domain-facing bootstrap pieces so the folder layout already matches the target architecture.

## First Delivered Behavior

- The app reads required settings from environment variables.
- The app exposes a `/health` endpoint that returns `200 OK` and basic service metadata.
- MongoDB client initialization is centralized and injectable through application state.
- Tests cover the bootstrapped HTTP path and configuration parsing.

## Error Handling

- Missing configuration should fail fast during startup.
- Mongo initialization errors should be surfaced as startup failures instead of being hidden.
- HTTP handlers remain thin and avoid embedding persistence logic.

## Testing Strategy

- Start with a failing test for the health route.
- Add a failing test for settings parsing from environment.
- Implement the minimum code needed to make both pass.
- Run `cargo test` after the bootstrap changes.

## Obsidian Handbook Scope

Create a new vault folder at `obsidian/andrzej.witkowski/` with a minimal handbook modeled after the existing project:

- `Getting Started.md`
- `opencode/OpenCode Start Here.md`
- `opencode/Project Constraints Index.md`
- `opencode/Agent Completion Checklist.md`
- `opencode/Architecture Design Restrictions.md`
- `opencode/Clean Code Rules.md`
- `opencode/Do and Don't Cheat Sheet.md`
- `opencode/Rust Backend Rules.md`
- `opencode/Mongo Reliability Rules.md`
- `opencode/HTTP Adapter Rules.md`

These notes should be Rust- and Mongo-specific, but preserve the same operational style: hexagonal boundaries, persist-before-side-effects, explicit recovery thinking, and verification-before-completion.
