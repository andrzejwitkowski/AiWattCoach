# Recent Workout Recaps in LLM Prompt Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Coach chat (`WorkoutSummaryChat`) ma widzieć **wszystkie workout recap-y** z ostatnich ~14 dni — jako osobną sekcję w packed context **oraz** poprawnie podlinkowane do treningów w `rd`. Prompt ma wskazać coachowi, żeby z nich korzystał i nie dopytywał o info już tam obecne.

**Architecture:** Naprawiamy root cause: `TrainingContextBuilder` ładuje RPE/recap po gołych `activity.id`, podczas gdy reszta appki (lista summary w UI) używa `CompletedWorkoutTargetUseCases` z aliasami (`preferred_workout_id`, `equivalent_workout_ids`). Dodajemy też niezależną listę recap-ów w volatile context (`wr`), żeby model miał je nawet gdy join do `rd[].w[]` zawiedzie.

**Tech Stack:** Rust, Mongo `workout_summary`, `training_context`, `workout_summary` target resolution, prompt w `workout_summary/prompt.rs`.

**Base branch:** `origin/main` @ `98b4382`

---

## Diagnoza ze screenshota (admin prompt preview)

Tydzień 2026-05-26 … 2026-06-01: **tylko 2026-05-27** ma RPE i recap snippet; pozostałe treningi mają metryki mocy, ale brak summary.

To nie wygląda na brak recap-ów w Mongo — to **join lookup**:

| Warstwa | Jak ładuje summary | Alias resolution |
|--------|---------------------|------------------|
| UI / `list_summaries` | `resolve_completed_workout_targets_in_scope` → wiele `lookup_workout_ids` | ✅ |
| `TrainingContextBuilder` | `activity.id` + `event.id` → `find_by_user_id_and_workout_ids` | ❌ |

Kod problemu:

```704:732:src/domain/training_context/service/mod.rs
// load_workout_recaps_by_workout_id — tylko surowe activity/event ids
```

vs

```136:185:src/domain/workout_summary/service/use_cases/read.rs
// resolve_list_summary_lookups_in_scope — preferred + equivalent ids
```

**User intent:** tylko `workout_recap` (summary per trening), nie cała konwersacja.

---

## Rozwiązanie (2 warstwy)

### Warstwa A — napraw join (RPE + recap w `rd[].w[]`)

Użyć tej samej logiki lookup co `list_summaries` dla wszystkich activity id z recent window.

### Warstwa B — osobna sekcja recap-ów (`wr`)

W `training_context_volatile` dodać tablicę wszystkich recap-ów z zakresu dat recent window:

```json
"wr": [
  {"d":"2026-05-27","id":"ride-1","rpe":2,"recap":"### Workout Recap: ..."},
  {"d":"2026-05-31","id":"...","rpe":7,"recap":"..."}
]
```

- Ładowane **niezależnie** od joinu per workout w `rd`
- Data `d` z `CompletedWorkout.start_date_local` (nie `saved_at`)
- Coach prompt explicite wskazuje na `wr` jako authoritative saved summaries

---

## File Structure

| Plik | Zmiana |
|------|--------|
| `src/domain/training_context/service/summary_lookup.rs` *(new)* | Wspólny loader: alias-aware batch lookup RPE/recap |
| `src/domain/training_context/service/mod.rs` | Inject optional `CompletedWorkoutTargetUseCases`; budowa `recent_workout_recaps` |
| `src/domain/training_context/model.rs` | `RecentWorkoutRecapContext`, pole na `TrainingContext` |
| `src/domain/training_context/packing/payloads/volatile.rs` | Serializacja `wr` |
| `src/domain/llm/context_prelude.rs` | Legenda: `recap`, `wr` |
| `src/domain/workout_summary/prompt.rs` | Instrukcja dla coacha + `current_workout_recap` |
| `src/main.rs` / wiring | Podłączyć `CompletedWorkoutTargetAdapter` do buildera (jak w workout summary service) |
| `frontend/.../decodePackedContext.ts` | Etykieta/dekoder dla `wr` (admin preview) |
| Testy buildera, packing, promptu | Patrz taski |

**Opcjonalnie (jeśli batch alias lookup wystarczy):** nowy port repo `list_recaps_for_user` — na start **nie** potrzebny; wystarczy alias-aware lookup + scan summaries z recap w oknie dat.

---

### Task 1: RED — test regresji alias lookup

**Files:**
- Modify: `src/domain/training_context/service/tests/builder/focus_and_aliases.rs`
- Modify: `src/domain/training_context/service/tests/support/` (fake target service jeśli brak)

- [ ] **Step 1: Test — dwa treningi w tygodniu, summary pod storage id, activity id inny**

Scenariusz jak w produkcji: 3+ completed workouts w recent window, summaries z recap pod `preferred_workout_id` / Wahoo id, activities w context używają canonical/Intervals id.

```rust
#[tokio::test]
async fn builder_links_recap_for_all_alias_backed_recent_workouts() {
    // assert every recent day with saved summary has workout_recap in rd[].w[]
}
```

- [ ] **Step 2: Run (expect FAIL on main)**

```bash
CARGO_BUILD_JOBS=1 cargo test builder_links_recap_for_all_alias -- --nocapture
```

---

### Task 2: Alias-aware summary lookup (Warstwa A)

**Files:**
- Create: `src/domain/training_context/service/summary_lookup.rs`
- Modify: `src/domain/training_context/service/mod.rs`
- Modify: `src/domain/training_context/service/context.rs` (jeśli sygnatura lookup się zmieni)

- [ ] **Step 1: Wyciągnij helper `resolve_summary_lookup_ids`**

Mirror logiki z `resolve_list_summary_lookups_in_scope`:
- input: `user_id`, `requested_workout_ids[]`, optional `CompletedWorkoutTargetUseCases`
- output: `HashMap<requested_id, WorkoutSummary>` (po `find_by_user_id_and_workout_ids` na **unii** wszystkich lookup ids)

Fallback gdy port niedostępny: obecne zachowanie (tylko raw ids).

- [ ] **Step 2: Zastąp `load_rpe_by_workout_id` i `load_workout_recaps_by_workout_id`**

Jedna funkcja `load_workout_summary_fields_by_workout_id` → `(rpe_map, recap_map)`.

- [ ] **Step 3: Run test Task 1 → PASS**

- [ ] **Step 4: Commit**

---

### Task 3: Osobna sekcja `wr` (Warstwa B)

**Files:**
- Modify: `src/domain/training_context/model.rs`
- Modify: `src/domain/training_context/service/mod.rs`
- Modify: `src/domain/training_context/service/summary_lookup.rs`

- [ ] **Step 1: Dodaj model**

```rust
pub struct RecentWorkoutRecapContext {
    pub date: String,           // YYYY-MM-DD z completed workout
    pub workout_id: String,     // frontend-visible id
    pub rpe: Option<u8>,
    pub recap: String,
}
// TrainingContext.recent_workout_recaps: Vec<RecentWorkoutRecapContext>
```

- [ ] **Step 2: Buduj listę po załadowaniu summaries**

Algorytm:
1. Weź wszystkie summaries z non-empty `workout_recap_text` znalezione przez alias-aware lookup dla **recent_activity_ids**
2. Dodatkowo: dla każdego summary z recap, resolve datę przez mapę `completed_workout_id → start_date_local` (z `history_completed_workouts` / recent activities)
3. Filtruj `date ∈ [recent_start, focus_date]`
4. Sortuj po dacie malejąco
5. Dedup po `(date, recap)` jeśli alias da duplikat

- [ ] **Step 3: Test**

```rust
#[tokio::test]
async fn builder_populates_recent_workout_recaps_section_for_date_range() { ... }
```

- [ ] **Step 4: Commit**

---

### Task 4: Packing + legenda + admin preview

**Files:**
- Modify: `src/domain/training_context/packing/payloads/volatile.rs`
- Modify: `src/domain/llm/context_prelude.rs`
- Modify: `src/domain/training_context/packing/tests.rs`
- Modify: `frontend/src/features/admin-prompt-preview/utils/decodePackedContext.ts`
- Modify: `frontend/src/features/admin-prompt-preview/components/DecodedPackedContext.tsx`

- [ ] **Step 1: `wr` w `VolatilePayload`**

```rust
#[serde(skip_serializing_if = "Vec::is_empty")]
wr: Vec<CompactWorkoutRecap<'a>>,
// { d, id, rpe?, recap }
```

- [ ] **Step 2: Legenda**

- `recap` — recap inline on a recent workout entry in `rd[].w[]` when linked
- `wr` — saved workout recaps for the recent window, listed separately by date; **prefer this as the complete set of saved summaries**; each entry is authoritative for that session

- [ ] **Step 3: Admin preview UI**

Nowa sekcja „Workout recaps (wr)” pod Recent Days — lista dat + recap (jak snippet).

- [ ] **Step 4: Test packing assert `"wr":[{`

- [ ] **Step 5: Commit**

---

### Task 5: Wiring + prompt coacha

**Files:**
- Modify: `src/domain/training_context/service/mod.rs` (builder constructor)
- Modify: `src/main.rs` (lub miejsce tworzenia buildera)
- Modify: `src/domain/workout_summary/prompt.rs`
- Modify: `tests/llm_adapters/coaching.rs`

- [ ] **Step 1: Podłącz `CompletedWorkoutTargetUseCases` do `DefaultTrainingContextBuilder`**

Ten sam adapter co `WorkoutSummaryService` (`CompletedWorkoutTargetAdapter`).

- [ ] **Step 2: Prompt — `WORKOUT_COACH_RECENT_WORKOUT_RECAP_PROMPT`**

> Saved workout summaries for recent sessions appear in packed context as `wr` (preferred) and optionally as `recap` on matching entries in `rd`. Treat them as already-known facts. Do not ask the athlete again for information clearly stated in `wr`, `recap`, or earlier messages in the current workout thread.

- [ ] **Step 3: `build_stable_context` — `current_workout_recap=` gdy istnieje**

- [ ] **Step 4: Test promptu**

- [ ] **Step 5: Commit**

---

### Task 6: Weryfikacja

- [ ] **Step 1: Targeted tests**

```bash
CARGO_BUILD_JOBS=1 cargo test training_context -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test llm_workout_coach -- --test-threads=1
bun run --cwd frontend test src/features/admin-prompt-preview/
```

- [ ] **Step 2: Quality gates**

```bash
bun run verify:arch
CARGO_BUILD_JOBS=1 cargo fmt --all --check
CARGO_BUILD_JOBS=1 cargo clippy --all-targets --all-features -- -D warnings
./scripts/rebuild_graphify.sh
```

- [ ] **Step 3: Manual smoke (Twój case)**

Admin prompt preview dla treningu z 2026-06-01:
- sekcja **Workout recaps (`wr`)** pokazuje recap-y dla 26–31 maj (jeśli zapisane w Mongo)
- `rd` też ma `recap` przy każdym powiązanym treningu
- coach nie pyta ponownie o wynik Szosomanii, jeśli jest w recap z 31.05

---

## Dwa możliwe wyjaśnienia brakujących recap-ów

Po fixie lookup, jeśli w `wr` nadal brakuje treningu:

1. **Summary nie było saved** → recap nie powstał (workflow save → `generate_recap_for_saved_workout`)
2. **Recap generation failed** → sprawdź `workout_recap_text` w Mongo dla tego `workout_id`

To diagnostyka po merge, nie blocker implementacji.

---

## Follow-up (osobny PR)

- Recap generator przy save: dołączyć konwersację coacha do inputu LLM, żeby wynik zawodów trafił do `workout_recap_text`
- Calendar coach: krótka wzmianka o `wr` w `calendar_coach_system_prompt()`

---

## Kryterium done

- [ ] Alias-aware lookup — RPE i recap w `rd[].w[]` dla wszystkich treningów z zapisanym summary (test regresji)
- [ ] Osobna sekcja `wr` z recap-ami z recent date range
- [ ] Legenda opisuje `recap` i `wr`
- [ ] Workout coach prompt wskazuje na `wr` i zakazuje re-dopytywania
- [ ] Admin preview pokazuje sekcję `wr`
- [ ] fmt, clippy, verify:arch, testy przechodzą
