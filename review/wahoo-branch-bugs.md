# Review: `docs/wahoo-first-completed-workout-plan` bugs

Review wykonany 2026-04-25, diff vs `origin/main`, 58 files.

---

## Bug 1: `advance_calendar_cursor` nie przesuwa kursora przy kolejnych pollach z eventami

**Plik:** `src/config/provider_polling/mod.rs:791-793`
**Severity:** Medium-High

Funkcja zmieniła sygnaturę z `advance_calendar_cursor(state, events, range)` na
`advance_calendar_cursor(state, range)`. Stare zachowanie przy niepustej liście
eventów zawsze zwracało `Some(range.newest.clone())`. Nowe zachowanie zwraca
istniejący `state.cursor` bez zmiany:

```rust
fn advance_calendar_cursor(state: &ProviderPollState, range: &DateRange) -> Option<String> {
    state.cursor.clone().or_else(|| Some(range.newest.clone()))
}
```

Test `poll_due_once_imports_calendar_events_and_advances_cursor` przechodzi tylko
dlatego, że używa świeżego stanu (`cursor = None`). Żaden test nie weryfikuje
przypadku kolejnego polla z niepustą listą eventów i istniejącym kursorem.
W produkcji, po pierwszym udanym pollu, kursor nie będzie się przesuwał, co oznacza
wielokrotne odczytywanie tego samego okna dat.

**Fix:** Albo zawsze zwracaj `Some(range.newest.clone())`, albo przywróć starą
sygnaturę i logikę zależną od eventów. Najprostsza poprawna wersja:

```rust
fn advance_calendar_cursor(_state: &ProviderPollState, range: &DateRange) -> Option<String> {
    Some(range.newest.clone())
}
```

bo teraz i tak zawsze importujemy eventy w poll_intervals_calendar_stream.

---

## Bug 2: Wahoo cursor nie przesuwa się, jeśli wszystkie workouty w stronie nie mają `workout_summary`

**Plik:** `src/config/provider_polling/mod.rs:357-437`
**Severity:** Low (skrajny corner case, ale warto zabezpieczyć)

Jeśli cała strona workoutów z Wahoo nie ma `workout_summary.is_some()`,
`workouts_to_import` zostaje puste, `newest_cursor` zostaje przy watermarku
i pętla zapętla się – za każdym razem odczytuje tę samą stronę.

```rust
for workout in list.workouts {
    // ...
    if workout.workout_summary.is_some() {
        workouts_to_import.push(workout);
    }
}
// ...
for workout in workouts_to_import.iter().rev() {
    // ...
    let updated_at = workout_sort_key(workout)?;
    newest_cursor = match newest_cursor {
        Some(current) => Some(std::cmp::max(current, updated_at)),
        None => Some(updated_at),
    };
}
```

Gdy `workouts_to_import` jest puste, `newest_cursor` nigdy nie jest aktualizowane
i zostaje przy `watermark`.

**Fix:** Przesuwaj kursor na podstawie `workout_sort_key` ostatniego workoutu
w stronie, nawet jeśli nie ma on summary. Np. przed breakiem w zewnętrznej pętli,
zawsze ustaw `newest_cursor` na `workout_sort_key` ostatniego elementu `list.workouts`.

---

## Bug 3: Osierocone stany `ProviderPollStream::Calendar` po deployu

**Plik:** `src/main_runtime.rs:150-192`
**Severity:** Low / uwaga migracyjna

`reconcile_intervals_poll_states` utrzymuje teraz tylko
`ProviderPollStream::CompletedWorkouts`. Istniejące stany `Calendar` nie są
czyszczone ani aktualizowane. Jeśli użytkownik miał TYLKO stan `Calendar`
– nie otrzyma nowego stanu `CompletedWorkouts` przy tym deployu, a stary stan
`Calendar` będzie wisiał jako osierocony.

Jest to zgodne z planem (Task 7: przestać importować zewnętrzne eventy
kalendarzowe), ale warto rozważyć:

- Dodanie `reconcile` stepu, który usuwa historyczne stany `Calendar`
  (albo ustawia im `next_due_at_epoch_seconds = i64::MAX`)
- Albo wpisanie w planie rolloutu, że osierocone stany są bezpieczne
  i nie powodują problemów wydajnościowych

---

## Podsumowanie

| # | Problem | Severity |
|---|---------|----------|
| 1 | `advance_calendar_cursor` nie przesuwa kursora | Medium-High |
| 2 | Wahoo cursor stuck gdy brak `workout_summary` | Low |
| 3 | Osierocone stany `Calendar` | Low |

Pozostała część brancha (autorytatywne wrappery, FIT enrichment flow, wiring
w `main.rs`, testy) jest solidna i nie zawiera znalezionych błędów produkcyjnych.
