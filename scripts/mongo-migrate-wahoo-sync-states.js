/*
Usage:

  Dry run (default):
    MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js

  Apply migration:
    MIGRATION_APPLY=true MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js

  Migrate one user only:
    MIGRATION_APPLY=true MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js

  Replace existing target rows for the same canonical entity:
    MIGRATION_APPLY=true MIGRATION_OVERWRITE_EXISTING=true MONGODB_DATABASE=aiwattcoach mongosh "$MONGODB_URI" --file scripts/mongo-migrate-wahoo-sync-states.js

Environment variables:
  MONGODB_DATABASE                Optional. Uses the current db when omitted.
  MIGRATION_SOURCE_COLLECTION     Optional. Default: planned_workout_wahoo_syncs
  MIGRATION_TARGET_COLLECTION     Optional. Default: external_sync_states
  MIGRATION_USER_ID               Optional. Limit migration to one user.
  MIGRATION_APPLY                 Optional. Set to true to write changes.
  MIGRATION_OVERWRITE_EXISTING    Optional. Set to true to replace existing target docs.

Notes:
  - Dry run is the default.
  - Existing target docs are skipped by default to avoid clobbering fresher data.
  - The script aborts on duplicate legacy Wahoo plan ids or workout tokens because the
    new collection enforces those lookups as unique for non-null values.
*/

(function migrateWahooSyncStates() {
  const config = readConfig();
  const database = resolveDatabase(config.databaseName);
  const source = database.getCollection(config.sourceCollection);
  const target = database.getCollection(config.targetCollection);
  const sourceFilter = buildSourceFilter(config.userId);

  printSection("Configuration");
  printjson({
    database: database.getName(),
    sourceCollection: config.sourceCollection,
    targetCollection: config.targetCollection,
    userId: config.userId,
    apply: config.apply,
    overwriteExisting: config.overwriteExisting,
  });

  const duplicateSummary = {
    planIdDuplicates: findLegacyDuplicates(source, sourceFilter, "wahoo_plan_id"),
    workoutTokenDuplicates: findLegacyDuplicates(source, sourceFilter, "wahoo_workout_token"),
  };

  if (duplicateSummary.planIdDuplicates.length > 0 || duplicateSummary.workoutTokenDuplicates.length > 0) {
    printSection("Blocked By Legacy Duplicates");
    printjson(duplicateSummary);
    print("Resolve duplicate legacy Wahoo identifiers before migrating into external_sync_states.");
    quit(1);
  }

  const cursor = source.find(sourceFilter).sort({ user_id: 1, planned_workout_id: 1 });
  const summary = {
    scanned: 0,
    skippedExisting: 0,
    toInsert: 0,
    toReplace: 0,
    bulkWriteOps: 0,
    matchedCount: 0,
    modifiedCount: 0,
    upsertedCount: 0,
    samples: [],
  };
  const operations = [];

  while (cursor.hasNext()) {
    const legacy = cursor.next();
    const migrated = mapLegacySyncDocument(legacy);
    const targetFilter = buildTargetFilter(migrated);
    const existing = target.findOne(targetFilter, { _id: 1 });

    summary.scanned += 1;

    if (existing && !config.overwriteExisting) {
      summary.skippedExisting += 1;
      pushSample(summary.samples, {
        action: "skip_existing",
        user_id: migrated.user_id,
        canonical_entity_id: migrated.canonical_entity_id,
      });
      continue;
    }

    if (existing) {
      summary.toReplace += 1;
    } else {
      summary.toInsert += 1;
    }

    pushSample(summary.samples, {
      action: existing ? "replace" : "insert",
      user_id: migrated.user_id,
      canonical_entity_id: migrated.canonical_entity_id,
      sync_status: migrated.sync_status,
      wahoo_plan_id: migrated.wahoo_plan_id,
      wahoo_workout_id: migrated.wahoo_workout_id,
    });

    if (!config.apply) {
      continue;
    }

    operations.push({
      replaceOne: {
        filter: targetFilter,
        replacement: migrated,
        upsert: true,
      },
    });

    if (operations.length >= 500) {
      flushOperations(target, operations, summary);
    }
  }

  if (config.apply) {
    flushOperations(target, operations, summary);
  }

  printSection(config.apply ? "Apply Summary" : "Dry Run Summary");
  printjson(summary);

  if (!config.apply) {
    print("Dry run only. Re-run with MIGRATION_APPLY=true to write changes.");
  }
})();

function readConfig() {
  return {
    databaseName: readEnv("MONGODB_DATABASE", null),
    sourceCollection: readEnv("MIGRATION_SOURCE_COLLECTION", "planned_workout_wahoo_syncs"),
    targetCollection: readEnv("MIGRATION_TARGET_COLLECTION", "external_sync_states"),
    userId: readEnv("MIGRATION_USER_ID", null),
    apply: readEnv("MIGRATION_APPLY", "false") === "true",
    overwriteExisting: readEnv("MIGRATION_OVERWRITE_EXISTING", "false") === "true",
  };
}

function readEnv(name, fallbackValue) {
  const value = process.env[name];
  return value === undefined || value === "" ? fallbackValue : value;
}

function resolveDatabase(databaseName) {
  return databaseName ? db.getSiblingDB(databaseName) : db;
}

function buildSourceFilter(userId) {
  if (!userId) {
    return {};
  }

  return { user_id: userId };
}

function buildTargetFilter(migrated) {
  return {
    user_id: migrated.user_id,
    provider: migrated.provider,
    canonical_entity_kind: migrated.canonical_entity_kind,
    canonical_entity_id: migrated.canonical_entity_id,
  };
}

function findLegacyDuplicates(source, sourceFilter, fieldName) {
  return source
    .aggregate([
      { $match: sourceFilter },
      { $match: { [fieldName]: { $exists: true, $ne: null } } },
      {
        $group: {
          _id: {
            user_id: "$user_id",
            value: `$${fieldName}`,
          },
          count: { $sum: 1 },
          planned_workout_ids: { $push: "$planned_workout_id" },
        },
      },
      { $match: { count: { $gt: 1 } } },
      { $sort: { "_id.user_id": 1 } },
    ])
    .toArray();
}

function mapLegacySyncDocument(legacy) {
  const lastSyncedAtEpochSeconds = resolveLegacyLastSyncedEpochSeconds(legacy);
  const includeRemoteSnapshot =
    legacy.status === "synced" || legacy.status === "failed";

  return {
    user_id: legacy.user_id,
    provider: "wahoo",
    canonical_entity_kind: "planned_workout",
    canonical_entity_id: legacy.planned_workout_id,
    external_id:
      legacy.wahoo_workout_id === undefined || legacy.wahoo_workout_id === null
        ? null
        : String(legacy.wahoo_workout_id),
    wahoo_plan_external_id: legacy.wahoo_plan_external_id ?? null,
    wahoo_plan_id: legacy.wahoo_plan_id ?? null,
    wahoo_workout_id: legacy.wahoo_workout_id ?? null,
    wahoo_workout_token: legacy.wahoo_workout_token ?? null,
    sync_status: mapLegacyStatus(legacy.status),
    last_synced_payload_hash: legacy.payload_hash ?? null,
    last_seen_remote_payload_hash: includeRemoteSnapshot ? legacy.payload_hash ?? null : null,
    last_error:
      legacy.status === "failed"
        ? legacy.last_error ?? "wahoo sync failed"
        : legacy.last_error ?? null,
    last_synced_at_epoch_seconds: lastSyncedAtEpochSeconds,
    last_synced_at: toDateOrNull(lastSyncedAtEpochSeconds),
    last_seen_remote_at_epoch_seconds: includeRemoteSnapshot ? lastSyncedAtEpochSeconds : null,
    last_seen_remote_at: includeRemoteSnapshot ? toDateOrNull(lastSyncedAtEpochSeconds) : null,
    conflict_status: legacy.status === "synced" ? "in_sync" : "unknown",
  };
}

function mapLegacyStatus(status) {
  switch (status) {
    case "synced":
      return "synced";
    case "failed":
      return "failed";
    case "pending":
    case "unsynced":
    case "modified":
      return "pending";
    default:
      throw new Error(`Unsupported legacy Wahoo sync status: ${status}`);
  }
}

function resolveLegacyLastSyncedEpochSeconds(legacy) {
  const lastSynced = resolveEpochSeconds(
    legacy.last_synced_at,
    legacy.last_synced_at_epoch_seconds,
  );

  if (lastSynced !== null) {
    return lastSynced;
  }

  if (legacy.status !== "synced") {
    return null;
  }

  return resolveEpochSeconds(legacy.updated_at, legacy.updated_at_epoch_seconds);
}

function resolveEpochSeconds(dateValue, epochValue) {
  if (epochValue !== undefined && epochValue !== null) {
    return epochValue;
  }

  if (dateValue === undefined || dateValue === null) {
    return null;
  }

  if (typeof dateValue.getTime === "function") {
    return Math.floor(dateValue.getTime() / 1000);
  }

  throw new Error(`Unsupported date value: ${tojson(dateValue)}`);
}

function toDateOrNull(epochSeconds) {
  return epochSeconds === null ? null : new Date(epochSeconds * 1000);
}

function flushOperations(target, operations, summary) {
  if (operations.length === 0) {
    return;
  }

  const result = target.bulkWrite(operations, { ordered: false });
  summary.bulkWriteOps += operations.length;
  summary.matchedCount += result.matchedCount ?? 0;
  summary.modifiedCount += result.modifiedCount ?? 0;
  summary.upsertedCount += result.upsertedCount ?? 0;
  operations.length = 0;
}

function pushSample(samples, sample) {
  if (samples.length < 10) {
    samples.push(sample);
  }
}

function printSection(title) {
  print(`\n=== ${title} ===`);
}
