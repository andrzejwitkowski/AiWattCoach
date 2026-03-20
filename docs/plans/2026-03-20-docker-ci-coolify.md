# Docker CI Coolify Implementation Plan

> This plan is intended to be implemented task by task, in the order listed below.

**Goal:** Add local Docker development, GitHub Actions CI, manual tag-based releases, and manual Coolify deployment support.

**Architecture:** Keep runtime packaging separate from application code. CI validates Rust and Docker paths on every change, while release and deploy remain explicit manual operations.

**Tech Stack:** Rust, Cargo, Docker, Docker Compose, GitHub Actions, Coolify webhook

---

## Task 1: Add container runtime files

**Files:**
- Create: `Dockerfile`
- Create: `.dockerignore`
- Create: `docker-compose.yml`

**Step 1: Create multi-stage Dockerfile**

Expected: release binary builds in a builder image and runs in a slim runtime image.

**Step 2: Create `.dockerignore`**

Expected: `target`, `.git`, and local env files stay out of build context.

**Step 3: Create local compose stack**

Expected: app and Mongo run together for local development.

## Task 2: Add CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Run Rust verification on push and pull_request**

Expected: `cargo test` runs in CI.

**Step 2: Validate Docker build**

Expected: Docker image builds successfully in CI.

## Task 3: Add manual release workflow

**Files:**
- Create: `.github/workflows/release-manual.yml`

**Step 1: Add workflow_dispatch input for version tag**

Expected: release is manually triggered with a version like `v0.1.0`.

**Step 2: Create tag and GitHub release**

Expected: workflow creates and pushes the tag, then opens a GitHub release.

## Task 4: Add manual Coolify deploy workflow

**Files:**
- Create: `.github/workflows/deploy-coolify-manual.yml`

**Step 1: Add manual deployment workflow**

Expected: workflow can be run manually against a chosen ref.

**Step 2: Optionally call Coolify webhook**

Expected: if webhook secret exists and user requests it, workflow triggers Coolify deployment.

## Task 5: Update developer docs

**Files:**
- Modify: `README.md`

**Step 1: Document local Docker usage**

Expected: README explains how to run Mongo + app locally.

**Step 2: Document release and deploy flow**

Expected: README explains manual tag releases and manual Coolify deployment.

## Task 6: Verify end-to-end

**Files:**
- Verify: `Dockerfile`
- Verify: `docker-compose.yml`
- Verify: `.github/workflows/*.yml`

**Step 1: Run backend tests**

Run: `cargo test`
Expected: PASS

**Step 2: Validate local Docker image build**

Run: `docker build -t aiwattcoach:test .`
Expected: PASS
