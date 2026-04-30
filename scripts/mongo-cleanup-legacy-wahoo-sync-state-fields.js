/*
Usage:

  Dry run (default):
    MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js

  Apply field cleanup:
    MIGRATION_APPLY=true MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js

  Cleanup one user only:
    MIGRATION_APPLY=true MIGRATION_USER_ID=user-1 MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-cleanup-legacy-wahoo-sync-state-fields.js

Environment variables:
  MONGODB_DATABASE            Optional. Uses the current db when omitted.
  MIGRATION_SOURCE_COLLECTION Optional. Default: planned_workout_wahoo_syncs
  MIGRATION_TARGET_COLLECTION Optional. Default: external_sync_states
  MIGRATION_USER_ID           Optional. Limit cleanup to one user.
  MIGRATION_APPLY             Optional. Set to true to write changes.

Safety:
  - Dry run is the default.
  - Unsets only legacy fields that have already been migrated into external_sync_states.
  - Touches only source rows whose migrated target row exists and matches the expected migrated shape.
  - Leaves unmatched or mismatched legacy rows unchanged and reports them.
*/

const LEGACY_FIELDS_TO_UNSET = [
  "payload_hash",
  "status",
  "wahoo_plan_external_id",
  "wahoo_plan_id",
  "wahoo_workout_id",
  "wahoo_workout_token",
  "last_error",
  "last_synced_at_epoch_seconds",
  "last_synced_at",
];

(function cleanupLegacyWahooSyncStateFields() {
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
    fieldsToUnset: LEGACY_FIELDS_TO_UNSET,
  });

  const summary = {
    scanned: 0,
    eligibleForUnset: 0,
    alreadyClean: 0,
    missingTarget: 0,
    mismatch: 0,
    bulkWriteOps: 0,
    matchedCount: 0,
    modifiedCount: 0,
    eligibleSamples: [],
    alreadyCleanSamples: [],
    missingTargetSamples: [],
    mismatchSamples: [],
  };
  const operations = [];
  const cursor = source.find(sourceFilter).sort({ user_id: 1, planned_workout_id: 1 });

  while (cursor.hasNext()) {
    const legacy = cursor.next();
    const expected = mapLegacySyncDocument(legacy);
    const actual = target.findOne(buildTargetFilter(expected));

    summary.scanned += 1;

    if (!actual) {
      summary.missingTarget += 1;
      pushSample(summary.missingTargetSamples, {
        legacy_id: legacy._id,
        user_id: legacy.user_id,
        planned_workout_id: legacy.planned_workout_id,
      });
      continue;
    }

    const diffs = diffMigratedDocument(expected, actual);
    if (diffs.length > 0) {
      summary.mismatch += 1;
      pushSample(summary.mismatchSamples, {
        legacy_id: legacy._id,
        user_id: legacy.user_id,
        planned_workout_id: legacy.planned_workout_id,
        diffs,
      });
      continue;
    }

    const unsetSpec = buildUnsetSpec(legacy);
    if (!unsetSpec) {
      summary.alreadyClean += 1;
      pushSample(summary.alreadyCleanSamples, {
        legacy_id: legacy._id,
        user_id: legacy.user_id,
        planned_workout_id: legacy.planned_workout_id,
      });
      continue;
    }

    summary.eligibleForUnset += 1;
    pushSample(summary.eligibleSamples, {
      legacy_id: legacy._id,
      user_id: legacy.user_id,
      planned_workout_id: legacy.planned_workout_id,
      fields: Object.keys(unsetSpec),
    });

    if (!config.apply) {
      continue;
    }

    operations.push({
      updateOne: {
        filter: { _id: legacy._id },
        update: { $unset: unsetSpec },
      },
    });

    if (operations.length >= 500) {
      flushOperations(source, operations, summary);
    }
  }

  if (config.apply) {
    flushOperations(source, operations, summary);
  }

  printSection(config.apply ? "Field Cleanup Summary" : "Field Cleanup Dry Run Summary");
  printjson(summary);

  if (!config.apply) {
    print("Dry run only. Re-run with MIGRATION_APPLY=true to unset eligible legacy fields.");
  }
})();

function readConfig() {
  return {
    databaseName: readEnv("MONGODB_DATABASE", null),
    sourceCollection: readEnv("MIGRATION_SOURCE_COLLECTION", "planned_workout_wahoo_syncs"),
    targetCollection: readEnv("MIGRATION_TARGET_COLLECTION", "external_sync_states"),
    userId: readEnv("MIGRATION_USER_ID", null),
    apply: readEnv("MIGRATION_APPLY", "false") === "true",
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

function buildUnsetSpec(legacy) {
  const unsetSpec = {};

  LEGACY_FIELDS_TO_UNSET.forEach((fieldName) => {
    if (Object.prototype.hasOwnProperty.call(legacy, fieldName)) {
      unsetSpec[fieldName] = "";
    }
  });

  return Object.keys(unsetSpec).length > 0 ? unsetSpec : null;
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
    last_seen_remote_at_epoch_seconds: includeRemoteSnapshot ? lastSyncedAtEpochSeconds : null,
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

function diffMigratedDocument(expected, actual) {
  const diffs = [];
  const actualLastSyncedAt = resolveEpochSeconds(
    actual.last_synced_at,
    actual.last_synced_at_epoch_seconds,
  );
  const actualLastSeenRemoteAt = resolveEpochSeconds(
    actual.last_seen_remote_at,
    actual.last_seen_remote_at_epoch_seconds,
  );

  compareField(diffs, "external_id", expected.external_id, actual.external_id ?? null);
  compareField(
    diffs,
    "wahoo_plan_external_id",
    expected.wahoo_plan_external_id,
    actual.wahoo_plan_external_id ?? null,
  );
  compareField(diffs, "wahoo_plan_id", expected.wahoo_plan_id, actual.wahoo_plan_id ?? null);
  compareField(
    diffs,
    "wahoo_workout_id",
    expected.wahoo_workout_id,
    actual.wahoo_workout_id ?? null,
  );
  compareField(
    diffs,
    "wahoo_workout_token",
    expected.wahoo_workout_token,
    actual.wahoo_workout_token ?? null,
  );
  compareField(diffs, "sync_status", expected.sync_status, actual.sync_status ?? null);
  compareField(
    diffs,
    "last_synced_payload_hash",
    expected.last_synced_payload_hash,
    actual.last_synced_payload_hash ?? null,
  );
  compareField(
    diffs,
    "last_seen_remote_payload_hash",
    expected.last_seen_remote_payload_hash,
    actual.last_seen_remote_payload_hash ?? null,
  );
  compareField(diffs, "last_error", expected.last_error, actual.last_error ?? null);
  compareField(
    diffs,
    "last_synced_at_epoch_seconds",
    expected.last_synced_at_epoch_seconds,
    actualLastSyncedAt,
  );
  compareField(
    diffs,
    "last_seen_remote_at_epoch_seconds",
    expected.last_seen_remote_at_epoch_seconds,
    actualLastSeenRemoteAt,
  );
  compareField(
    diffs,
    "conflict_status",
    expected.conflict_status,
    actual.conflict_status ?? null,
  );

  return diffs;
}

function compareField(diffs, fieldName, expected, actual) {
  if (expected !== actual) {
    diffs.push({ field: fieldName, expected, actual });
  }
}

function flushOperations(source, operations, summary) {
  if (operations.length === 0) {
    return;
  }

  const result = source.bulkWrite(operations, { ordered: false });
  summary.bulkWriteOps += operations.length;
  summary.matchedCount += result.matchedCount ?? 0;
  summary.modifiedCount += result.modifiedCount ?? 0;
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
