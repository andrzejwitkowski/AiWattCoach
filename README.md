# AiWattCoach

Rust-based coaching backend for Intervals.icu sync, AI-assisted training planning, and future Wahoo delivery through Intervals planned workouts.

The repository now also includes a frontend shell in `frontend/` built with Bun, Vite, React, and Tailwind.

Production Docker and Coolify now build and run the Rust API plus the compiled SPA in one container. The backend serves the built files from `frontend/dist`.

## Local development

### Run with Docker Compose

```bash
docker compose up --build
```

This starts:
- app plus built frontend UI on `http://localhost:3002`
- MongoDB on `mongodb://127.0.0.1:27017`

Compose waits for MongoDB readiness before starting the app and exposes:
- `/health` for liveness
- `/ready` for readiness against the configured Mongo database

### Run locally without Docker

Copy `.env.example` to `.env` and set values as needed, then run:

```bash
bun install
cargo test
cargo run
```

The backend loads `.env` automatically from the repo root during local startup.

`bun install` also runs the Husky `prepare` script and installs the local git hooks for this repo.

Backend auth-related environment variables:

- `GOOGLE_OAUTH_CLIENT_ID`
- `GOOGLE_OAUTH_CLIENT_SECRET`
- `GOOGLE_OAUTH_REDIRECT_URL`
- `SESSION_COOKIE_NAME`
- `SESSION_TTL_HOURS`
- `SESSION_COOKIE_SECURE`
- `ADMIN_EMAILS` (comma-separated list, optional)

### Run the frontend shell

In a second terminal, copy `frontend/.env.example` to `frontend/.env` only if you need to override the API origin with a directly reachable backend or gateway, then run:

```bash
bun install --cwd frontend
bun run --cwd frontend dev
```

The frontend runs on `http://localhost:5173`. By default it uses same-origin requests, and the Vite dev proxy forwards `GET /health` and `GET /ready` to the backend on `http://127.0.0.1:3002`.

This Vite server setup is for local development only. In Docker and Coolify, Bun builds the SPA during the image build and the backend serves the compiled files from the same origin as the API.

If you set `VITE_API_BASE_URL`, point it at an origin the browser can reach directly, or expose the backend through the same public origin via a reverse proxy.

The first integrated UI path uses the real backend endpoints:

- `GET /health`
- `GET /ready`

The app shell shows backend connectivity state and exposes a dedicated settings/configuration entry point.

## CI

GitHub Actions runs:
- `cargo fmt -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`
- `docker build -t aiwattcoach:ci .`

on pull requests and pushes to `main` or `feature/**` branches.

For local end-to-end verification, run:

```bash
bun install
bun install --cwd frontend
bun run verify:rust
bun run test:all
```

Git hooks enforce part of this automatically:

- `pre-commit` runs `bun run verify:rust` when staged Rust files are present
- `pre-push` runs `bun run verify:all`

## Releases

Manual release flow lives in GitHub Actions:
- run `Release Manual`
- provide version in format `vX.Y.Z`
- workflow creates git tag and GitHub Release

## Coolify deployment

Deployment is manual for now.

`docker-compose.yml` is for local development only. Coolify should build the single `Dockerfile` image that serves both the API and the frontend UI.

Use the `Deploy Coolify Manual` workflow to:
- validate Docker build for the workflow ref
- optionally trigger the Coolify webhook configured in `COOLIFY_WEBHOOK_URL`

When webhook triggering is enabled, the workflow also checks `COOLIFY_DEPLOY_REF` so GitHub-side validation matches the branch Coolify is configured to deploy.

If you prefer, you can also deploy directly from Coolify against the branch or tag configured there.

### Coolify environment variables

Set these in Coolify for the single application container:

- `APP_NAME=AiWattCoach`
- `SERVER_HOST=0.0.0.0`
- `SERVER_PORT=3002`
- `MONGODB_URI=<paste Mongo URL (internal) from the Coolify Mongo resource>`
- `MONGODB_DATABASE=<database name configured in the Coolify Mongo resource>`

### Coolify setup from public GitHub repo

Create the application in Coolify from the public GitHub repository and select `Dockerfile` as the build method.

Coolify builds one image from the repo. That image compiles the frontend, builds the Rust binary, and runs one container that serves both the API and the SPA on port `3002`.

Use these values in the application settings:

- Branch: the test branch you want to deploy
- Dockerfile path: `Dockerfile`
- Port / Exposed port / Public port: `3002`
- Health check path: `/health`

Then set these environment variables in the application:

```env
APP_NAME=AiWattCoach
SERVER_HOST=0.0.0.0
SERVER_PORT=3002
MONGODB_URI=<paste the exact Mongo URL (internal) from Coolify>
MONGODB_DATABASE=<database name from the Coolify Mongo resource>
```

Notes:

- `MONGODB_URI` should be copied 1:1 from the Mongo resource `Mongo URL (internal)` field.
- `MONGODB_DATABASE` should match the database name shown in the Mongo resource configuration.
- If the database named by `MONGODB_DATABASE` does not exist yet, the app creates it on startup by creating a technical `_bootstrap` collection.
- If the Mongo resource uses TLS parameters in that URL, keep them exactly as generated by Coolify.
- If the app starts but `/ready` returns `503`, the issue is usually `MONGODB_URI` or `MONGODB_DATABASE`.
- The same public origin serves both the frontend UI and the API, so no separate frontend service is needed in Coolify.
- The runtime image includes `wget`, and the container healthcheck probes `/health` with `wget` for Coolify compatibility.

### GitHub Actions secrets for manual deploy

- `COOLIFY_WEBHOOK_URL=<Coolify deployment webhook>`
- `COOLIFY_DEPLOY_REF=<branch configured in Coolify>`

### Recommended manual flow

1. Merge work into `main` when ready.
2. Run `Release Manual` with a version like `v0.1.0`.
3. In Coolify, deploy the branch or tag you want.
4. Optionally run `Deploy Coolify Manual` if you later configure `COOLIFY_WEBHOOK_URL`.
