# Calendar Entry Integrity Issues

This note explains when `CalendarEntryIntegrityIssue` variants are emitted by
`verify_calendar_entry_integrity(...)` in `src/domain/calendar_view/integrity.rs`.

| Enum variant | When it appears | Why it exists |
| --- | --- | --- |
| `DuplicateEntry { entry_id, count }` | The `actual` slice contains the same `entry_id` more than once. | The read model should have at most one row per stable `entry_id`. Duplicates usually mean the projection maintenance logic failed to replace or deduplicate rows correctly. |
| `MissingEntry { entry_id }` | An `entry_id` exists in `expected` but there is no matching row in `actual`. It is also emitted when a row with the same `entry_id` exists in `actual` but has the wrong `entry_kind`. | From the integrity checker's point of view, the expected canonical entry is missing from the persisted view. A type mismatch is treated as a missing correct row plus a more specific mismatch signal. |
| `TypeMismatch { entry_id, expected_kind, actual_kind }` | A row with the same `entry_id` exists in both `expected` and `actual`, but the `entry_kind` differs. | This means the persisted view row points at the wrong semantic type for the same stable ID. The checker emits this in addition to `MissingEntry`, because the correct typed row is still absent. |
| `OrphanEntry { entry_id }` | An `entry_id` exists in `actual` but does not exist in `expected`. | The persisted view contains a stale or stray row that no longer corresponds to the canonical sources used to rebuild the expected set. |

## Why Type Mismatch Produces Two Issues

For a type mismatch, the current checker intentionally emits:

1. `MissingEntry`
2. `TypeMismatch`

Reason:
- `MissingEntry` says the correct expected row is not present in the persisted view.
- `TypeMismatch` adds the specific explanation that a row with the same ID exists but has the wrong kind.

This makes the report useful both for simple counting and for more detailed diagnosis.
