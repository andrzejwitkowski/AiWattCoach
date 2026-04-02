# Mongo Log Noise Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce noisy MongoDB local-dev connection chatter in Docker compose while preserving warnings, errors, and startup visibility.

**Architecture:** Add a `mongod` command override to the local compose Mongo service definitions. Use MongoDB log component verbosity settings to quiet connection-level `NETWORK` and `ACCESS` noise rather than changing the app or removing Mongo logs entirely.

**Tech Stack:** Docker Compose, MongoDB

---

### Task 1: Add local Mongo verbosity overrides to compose files

**Files:**
- Modify: `docker-compose-dev.yml`
- Modify: `docker-compose.yml`

**Step 1: Write minimal implementation**

Add a `command:` to both `mongo` services so `mongod` starts with reduced connection-chatter verbosity while keeping normal functionality.

**Step 2: Validate the compose files**

Run: `docker compose -f docker-compose-dev.yml config`

Run: `docker compose -f docker-compose.yml config`

Expected: both commands succeed.

### Task 2: Confirm formatting and startup shape

**Files:**
- No additional code changes expected

**Step 1: Review rendered compose output**

Confirm the `mongo` service still uses the expected image, healthcheck, volume, and new command override.

Expected: compose output is valid and startup-ready.
