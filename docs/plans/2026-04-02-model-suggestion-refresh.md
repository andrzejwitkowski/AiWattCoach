# Model Suggestion Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh the AI provider suggestion chips and default autofill model values using current provider model names while preserving freeform custom model input.

**Architecture:** Keep the change local to the AI settings frontend. Update the curated provider suggestion arrays in `AiAgentsCard.tsx`, then adjust the existing frontend tests that assert suggested model chips or provider-change autofill behavior.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Refresh curated provider suggestion lists

**Files:**
- Modify: `frontend/src/features/settings/components/AiAgentsCard.tsx`

**Step 1: Write minimal implementation**

Replace the stale hardcoded model suggestions with current curated examples for OpenAI, Gemini, and OpenRouter. Keep the freeform input behavior unchanged.

**Step 2: Verify the UI still allows custom models**

Confirm the model field remains a normal text input and suggestion chips only fill convenience values.

### Task 2: Update focused frontend tests

**Files:**
- Modify: `frontend/src/features/settings/components/AiAgentsCard.test.tsx`

**Step 1: Update assertions for new suggestions/defaults**

Adjust tests that check provider autofill defaults or suggested chip rendering so they match the refreshed curated lists.

**Step 2: Run focused frontend verification**

Run: `bun run --cwd frontend test src/features/settings/components/AiAgentsCard.test.tsx`

Expected: PASS.
