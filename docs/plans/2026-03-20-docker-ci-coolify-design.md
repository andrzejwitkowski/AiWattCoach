# Docker CI Coolify Design

## Goal

Add a local Docker development workflow, GitHub Actions for test/build/release, tag-based manual releases, and a manual deployment path for Coolify.

## Recommended Approach

Keep the deployment flow simple and reversible. Use `docker-compose.yml` for local development, a multi-stage `Dockerfile` for runtime builds, a CI workflow that validates Rust tests and Docker image creation, a manual release workflow that creates version tags and GitHub releases, and a separate manual Coolify deployment workflow that can optionally trigger a Coolify webhook.

## Why This Fits

- Coolify can build from the repo and does not require us to automate production deploys yet.
- Manual release and deploy steps reduce accidental shipping while the project is still being shaped.
- Tag-based releases keep versioning explicit without coupling it to every merge.

## Components

- `Dockerfile`: multi-stage Rust build, small runtime image
- `.dockerignore`: keep build context lean
- `docker-compose.yml`: local app + MongoDB for development
- `.github/workflows/ci.yml`: test and Docker build validation on push/PR
- `.github/workflows/release-manual.yml`: manual tag and GitHub release
- `.github/workflows/deploy-coolify-manual.yml`: manual deploy trigger with optional webhook
- `README.md`: local dev, release, and deploy instructions

## Safety Rules

- CI must fail if `cargo test` fails.
- Release workflow must require manual invocation and validate tag format.
- Deploy workflow must stay manual and not mutate runtime state unless explicitly requested.
- Coolify webhook use must be secret-driven and optional.
