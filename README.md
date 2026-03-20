# AiWattCoach

Rust-based coaching backend for Intervals.icu sync, AI-assisted training planning, and future Wahoo delivery through Intervals planned workouts.

## Local development

### Run with Docker Compose

```bash
docker compose up --build
```

This starts:
- app on `http://localhost:3000`
- MongoDB on `mongodb://localhost:27017`

Compose waits for MongoDB readiness before starting the app and exposes:
- `/health` for liveness
- `/ready` for readiness against app state

### Run locally without Docker

Copy `.env.example` to `.env` and set values as needed, then export the variables into your shell before running the app:

```bash
set -a
source .env
set +a
cargo test
cargo run
```

## CI

GitHub Actions runs:
- `cargo test`
- `docker build -t aiwattcoach:ci .`

on pushes and pull requests.

## Releases

Manual release flow lives in GitHub Actions:
- run `Release Manual`
- provide version in format `vX.Y.Z`
- workflow creates git tag and GitHub Release

## Coolify deployment

Deployment is manual for now.

`docker-compose.yml` is for local development only. Do not reuse it as the production topology for Coolify.

Use the `Deploy Coolify Manual` workflow to:
- validate Docker build for a chosen ref
- optionally trigger the Coolify webhook configured in `COOLIFY_WEBHOOK_URL`

The selected ref is used for GitHub-side validation. The webhook triggers whatever source Coolify is currently configured to deploy.

If you prefer, you can also deploy directly from Coolify against the branch or tag configured there.

### Coolify environment variables

Set these in Coolify for the container:

- `APP_NAME=AiWattCoach`
- `SERVER_HOST=0.0.0.0`
- `SERVER_PORT=3000`
- `MONGODB_URI=<your mongo connection string>`
- `MONGODB_DATABASE=aiwattcoach`

### Recommended manual flow

1. Merge work into `main` when ready.
2. Run `Release Manual` with a version like `v0.1.0`.
3. In Coolify, deploy the branch or tag you want.
4. Optionally run `Deploy Coolify Manual` if you later configure `COOLIFY_WEBHOOK_URL`.
