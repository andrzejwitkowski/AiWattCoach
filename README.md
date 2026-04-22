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

For the observability stack in local Docker, start the separate logging compose too:

```bash
docker compose -f docker-compose.logging.yml up -d
```

Then set `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317` if you run the backend directly on the host, or `http://alloy:4317` when the app runs in the same Docker network as the logging stack.

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
- `SESSION_COOKIE_SAME_SITE`
- `SESSION_TTL_HOURS`
- `SESSION_COOKIE_SECURE`
- `ADMIN_EMAILS` (comma-separated list, optional)

Cookie notes:

- Default local and same-origin setup uses `SESSION_COOKIE_SAME_SITE=lax`.
- If the frontend is served from a different site and uses an absolute `VITE_API_BASE_URL`, set `SESSION_COOKIE_SAME_SITE=none` and `SESSION_COOKIE_SECURE=true`.
- Browsers reject `SameSite=None` cookies that are not also `Secure`.

### Run the frontend shell

In a second terminal, copy `frontend/.env.example` to `frontend/.env` only if you need to override the API origin with a directly reachable backend or gateway, then run:

```bash
bun install --cwd frontend
bun run --cwd frontend dev
```

The frontend runs on `http://localhost:5173`. By default it uses same-origin requests, and the Vite dev proxy forwards `GET /health` and `GET /ready` to the backend on `http://127.0.0.1:3002`.

This Vite server setup is for local development only. In Docker and Coolify, Bun builds the SPA during the image build and the backend serves the compiled files from the same origin as the API.

If you set `VITE_API_BASE_URL`, point it at an origin the browser can reach directly, or expose the backend through the same public origin via a reverse proxy.

## Graphify

This repo includes checked-in graph artifacts in `graphify-out/` for agent navigation.

Primary entry points:

- `graphify-out/GRAPH_REPORT.md`
- `graphify-out/wiki/index.md`

To refresh the graph locally, install graphify first. The PyPI package name is `graphifyy`, while the CLI command is `graphify`.

```bash
pipx install graphifyy
graphify install --platform opencode
graphify hook install
```

Then regenerate or query from the repo root:

```bash
./scripts/rebuild_graphify.sh
graphify query "show the calendar and races flow"
```

`./scripts/rebuild_graphify.sh` prefers `GRAPHIFY_PYTHON`, then a valid executable path from `graphify-out/.graphify_python`, and finally the default `pipx` venv at `~/.local/pipx/venvs/graphifyy/bin/python`.

`.graphifyignore` excludes generated outputs and build directories so the graph does not index its own artifacts.

The first integrated UI path uses the real backend endpoints:

- `GET /health`
- `GET /ready`

The app shell shows backend connectivity state and exposes a dedicated settings/configuration entry point.

## CI

GitHub Actions runs:
- `bun run verify:arch`
- `cargo fmt -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`
- `docker build -t aiwattcoach:ci .` on pull requests and non-`main` pushes

on pull requests and pushes to `main` or `feature/**` branches.

On every successful push to `main`, the same workflow also:
- reuses the existing `vX.Y.Z` tag on `HEAD`, or creates the next patch tag from the highest existing release tag
- pushes that git tag to the repository
- publishes `registry.wattly.pl/aiwattcoach:vX.Y.Z`
- publishes `registry.wattly.pl/aiwattcoach:latest`

For local end-to-end verification, run:

```bash
bun install
bun install --cwd frontend
rustup toolchain install nightly-2026-01-22
rustup component add --toolchain nightly-2026-01-22 rust-src rustc-dev llvm-tools-preview
cargo +nightly-2026-01-22 install cargo_pup --version 0.1.7 --locked
bun run verify:arch
bun run verify:rust
bun run test:all
```

Git hooks enforce part of this automatically:

- `pre-commit` runs `bun run verify:rust` and `bun run verify:arch` when staged Rust files are present
- `pre-push` runs `bun run verify:all`

## Releases

The only supported release flow is `push` to `main`.

After CI passes on `main`, GitHub Actions automatically:
- allocates the next patch release tag in format `vX.Y.Z`
- pushes that tag to the repository
- builds the production image from `Dockerfile`
- pushes the image to `registry.wattly.pl/aiwattcoach` under both `vX.Y.Z` and `latest`

Required GitHub Actions secrets:
- `DOCKER_REGISTRY_USERNAME`
- `DOCKER_REGISTRY_PASSWORD`

For a manual fallback publish from your own machine:

```bash
docker login registry.wattly.pl
bun run docker:publish:registry -- v1.0.1
```

## Coolify deployment

Deployment stays manual, but Coolify should now pull a prebuilt image from the Docker registry instead of building from GitHub.

`docker-compose.yml` is still for local development only.

### Coolify environment variables

Set these in Coolify for the single application container:

- `APP_NAME=AiWattCoach`
- `SERVER_HOST=0.0.0.0`
- `SERVER_PORT=3002`
- `MONGODB_URI=<paste Mongo URL (internal) from the Coolify Mongo resource>`
- `MONGODB_DATABASE=<database name configured in the Coolify Mongo resource>`

### Coolify logging stack as a separate compose resource

If you want Grafana, Tempo, Loki, and Alloy in the same Coolify environment as an already-deployed app, deploy `docker-compose.logging.yml` as a separate resource.

Use this split model:

- existing app resource: image service using `registry.wattly.pl/aiwattcoach:latest`
- logging resource: `docker-compose.logging.yml`
- same Coolify environment/network for both resources
- app OTLP endpoint: `http://alloy:4317`

Set these env vars on the existing app resource:

```env
OTEL_SERVICE_NAME=aiwattcoach-backend
OTEL_EXPORTER_OTLP_ENDPOINT=http://alloy:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
```

Notes:

- This split preserves OTLP tracing to Tempo without cross-resource shared volumes.
- The current backend exports traces over OTLP; it does not yet export backend logs over OTLP.
- Loki and Grafana are still deployed and ready, but backend file-log ingestion is no longer part of the split Coolify setup.
- If you later want backend logs in Loki in Coolify, add OTLP log export from the app or reintroduce a shared-volume/file-scrape design.
- Expose Grafana from the logging resource if you want a public dashboard entry point.

### Coolify setup from Docker registry

Create the application in Coolify as an image-based service that pulls:

- image: `registry.wattly.pl/aiwattcoach:latest`
- port / exposed port / public port: `3002`
- health check path: `/health`

Then set these environment variables in the application:

```env
APP_NAME=AiWattCoach
SERVER_HOST=0.0.0.0
SERVER_PORT=3002
MONGODB_URI=<paste the exact Mongo URL (internal) from Coolify>
MONGODB_DATABASE=<database name from the Coolify Mongo resource>
```

Registry notes:

- configure `registry.wattly.pl` as a private registry in Coolify
- add the registry pull credentials there before the first deploy
- after GitHub publishes a new `latest`, trigger redeploy manually in Coolify when you want to roll it out
- for rollback, pin the image tag to an earlier `vX.Y.Z` instead of `latest`

Runtime notes:

- `MONGODB_URI` should be copied 1:1 from the Mongo resource `Mongo URL (internal)` field.
- `MONGODB_DATABASE` should match the database name shown in the Mongo resource configuration.
- If the database named by `MONGODB_DATABASE` does not exist yet, the app creates it on startup by creating a technical `_bootstrap` collection.
- If the Mongo resource uses TLS parameters in that URL, keep them exactly as generated by Coolify.
- If the app starts but `/ready` returns `503`, the issue is usually `MONGODB_URI` or `MONGODB_DATABASE`.
- The same public origin serves both the frontend UI and the API, so no separate frontend service is needed in Coolify.
- The runtime image includes `wget`, and the container healthcheck probes `/health` with `wget` for Coolify compatibility.

### Recommended release flow

1. Merge work into `main` when ready.
2. Wait for GitHub Actions to publish the new image tags.
3. In Coolify, redeploy the service when you want to roll out the newest `latest` image.
4. If needed, roll back by selecting an older `vX.Y.Z` image tag.
