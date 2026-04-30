# Wahoo Sync State Migration

This note describes the repository scripts used to migrate legacy Wahoo planned-workout sync rows from `planned_workout_wahoo_syncs` into `external_sync_states`.

## Files

- `scripts/mongo-migrate-wahoo-sync-states.js`
- `scripts/mongo-verify-wahoo-sync-states.js`
- `scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js`
- `scripts/mongo-cleanup-legacy-wahoo-sync-states.js`

## Collections

- Source: `planned_workout_wahoo_syncs`
- Target: `external_sync_states`

## Default Behavior

- Migration runs in dry-run mode unless `MIGRATION_APPLY=true` is set.
- Verification is read-only.
- Cleanup runs in dry-run mode unless `MIGRATION_APPLY=true` is set.
- Scripts can be scoped to one user with `MIGRATION_USER_ID=<user-id>`.

## Recommended Order

1. Dry run the migration.
2. Apply the migration.
3. Verify the migrated rows.
4. Optional: dry run the field-only cleanup.
5. Optional: apply the field-only cleanup.
6. Dry run the row cleanup.
7. Apply the row cleanup only after verification is clean.

## Commands

### 1. Migration Dry Run

```bash
MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js
```

### 2. Apply Migration

```bash
MIGRATION_APPLY=true MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js
```

### 3. Verify Migration

```bash
MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-verify-wahoo-sync-states.js
```

### 4. Cleanup Dry Run

Field-only cleanup dry run:

```bash
MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js
```

### 5. Apply Field Cleanup

```bash
MIGRATION_APPLY=true MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js
```

### 6. Cleanup Dry Run

```bash
MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-states.js
```

### 7. Apply Cleanup

```bash
MIGRATION_APPLY=true MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-states.js
```

## Scoped Run For One User

```bash
MIGRATION_APPLY=true MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js
```

```bash
MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-verify-wahoo-sync-states.js
```

```bash
MIGRATION_APPLY=true MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js
```

```bash
MIGRATION_APPLY=true MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-states.js
```

## Notes

- The migration aborts if the source collection contains duplicate non-null `wahoo_plan_id` or `wahoo_workout_token` values for the same user, because the target collection now enforces those lookups as unique.
- By default the migration skips target rows that already exist for the same `(user_id, provider, canonical_entity_kind, canonical_entity_id)` key. Use `MIGRATION_OVERWRITE_EXISTING=true` only if you explicitly want to replace already-migrated rows.
- Field-only cleanup unsets only the migrated Wahoo sync payload fields from legacy rows and leaves identifier/audit fields like `planned_workout_id`, `operation_key`, `date`, `source_workout_id`, `created_at*`, and `updated_at*` in place.
- Cleanup deletes only source rows whose corresponding migrated target row exists and matches the expected migrated shape. Rows without a matching target row, or rows whose migrated document differs, are left in place and reported.
