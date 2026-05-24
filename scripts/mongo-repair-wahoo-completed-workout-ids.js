/*
Usage:

  Dry run (default):
    MIGRATION_ID_MAPPINGS='[{"user_id":"user-1","bad_wahoo_workout_id":409097757,"good_wahoo_workout_id":459105036}]' \
      MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-repair-wahoo-completed-workout-ids.js

  Apply changes:
    MIGRATION_APPLY=true \
    MIGRATION_ID_MAPPINGS='[{"user_id":"user-1","bad_wahoo_workout_id":409097757,"good_wahoo_workout_id":459105036}]' \
      MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-repair-wahoo-completed-workout-ids.js

Optional environment variables:
  MONGODB_DATABASE         Optional. Uses the current db when omitted.
  MIGRATION_ID_MAPPINGS    Required unless DEFAULT_ID_MAPPINGS below is edited.
  MIGRATION_APPLY          Optional. true/false. Default: false.
  MIGRATION_ALLOW_PARTIAL  Optional. true/false. Default: false.
  MIGRATION_MAX_SAMPLES    Optional. Positive integer. Default: 10.

Safety:
  - Dry run is the default.
  - Requires explicit verified summary-id -> workout-id mappings.
  - Repairs only completed-workout identity stores plus workout-summary state keyed by that id.
  - Detects and blocks on unique-key collisions.
  - Detects unsupported training-plan state that would become orphaned after a partial repair.
*/

const DEFAULT_ID_MAPPINGS = [];

(function repairWahooCompletedWorkoutIds() {
  const config = readConfig();
  const mappings = readMappings();
  const database = resolveDatabase(config.databaseName);
  const collections = buildCollections(database);

  printSection("Configuration");
  printjson({
    database: database.getName(),
    apply: config.apply,
    allowPartial: config.allowPartial,
    maxSamples: config.maxSamples,
    mappingCount: mappings.length,
  });

  const results = mappings.map((rawMapping) =>
    processMapping(collections, normalizeMapping(rawMapping), config),
  );

  printSection(config.apply ? "Repair Summary" : "Repair Dry Run Summary");
  printjson(buildSummary(results));

  printSection("Per-Mapping Results");
  print(EJSON.stringify(results, null, 2));

  if (!config.apply) {
    print(
      "Dry run only. Re-run with MIGRATION_APPLY=true after reviewing blockers and planned updates.",
    );
  }
})();

function readConfig() {
  return {
    databaseName: readEnv("MONGODB_DATABASE", null),
    apply: readBool("MIGRATION_APPLY", false),
    allowPartial: readBool("MIGRATION_ALLOW_PARTIAL", false),
    maxSamples: readPositiveInt("MIGRATION_MAX_SAMPLES", 10),
  };
}

function readMappings() {
  const raw = readEnv("MIGRATION_ID_MAPPINGS", null);
  const parsed = raw === null ? DEFAULT_ID_MAPPINGS : EJSON.parse(raw);

  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error(
      "Provide at least one mapping via MIGRATION_ID_MAPPINGS or edit DEFAULT_ID_MAPPINGS.",
    );
  }

  return parsed;
}

function resolveDatabase(databaseName) {
  return databaseName ? db.getSiblingDB(databaseName) : db;
}

function buildCollections(database) {
  return {
    completedWorkouts: database.getCollection("completed_workouts"),
    externalObservations: database.getCollection("external_observations"),
    externalSyncStates: database.getCollection("external_sync_states"),
    tasks: database.getCollection("tasks"),
    wahooFitFiles: database.getCollection("wahoo_fit_files"),
    workoutSummaries: database.getCollection("workout_summaries"),
    llmReplyOperations: database.getCollection("llm_reply_operations"),
    plannedCompletedLinks: database.getCollection("planned_completed_workout_links"),
    calendarEntryViews: database.getCollection("calendar_entry_views"),
    trainingPlanOperations: database.getCollection("training_plan_generation_operations"),
    trainingPlanSnapshots: database.getCollection("training_plan_snapshots"),
    trainingPlanProjectedDays: database.getCollection("training_plan_projected_days"),
  };
}

function normalizeMapping(raw) {
  const userId = expectString(raw.user_id, "mapping.user_id");
  const badWahooWorkoutId = expectInteger(
    raw.bad_wahoo_workout_id,
    "mapping.bad_wahoo_workout_id",
  );
  const goodWahooWorkoutId = expectInteger(
    raw.good_wahoo_workout_id,
    "mapping.good_wahoo_workout_id",
  );

  if (badWahooWorkoutId === goodWahooWorkoutId) {
    throw new Error(
      `mapping for user '${userId}' has identical bad/good Wahoo workout ids`,
    );
  }

  const badActivityId = String(badWahooWorkoutId);
  const goodActivityId = String(goodWahooWorkoutId);
  const badCompletedWorkoutId = `wahoo-workout:${badActivityId}`;
  const goodCompletedWorkoutId = `wahoo-workout:${goodActivityId}`;
  const oldWorkoutSummaryScopeKey = `workout-summary:${userId}:${badCompletedWorkoutId}`;
  const newWorkoutSummaryScopeKey = `workout-summary:${userId}:${goodCompletedWorkoutId}`;
  const oldTrainingPlanPrefix = `training-plan:${userId}:${badCompletedWorkoutId}:`;

  return {
    userId,
    badWahooWorkoutId,
    goodWahooWorkoutId,
    badActivityId,
    goodActivityId,
    badCompletedWorkoutId,
    goodCompletedWorkoutId,
    oldWahooFitDedupeKey: `wahoo-fit:${badCompletedWorkoutId}`,
    newWahooFitDedupeKey: `wahoo-fit:${goodCompletedWorkoutId}`,
    oldWorkoutSummaryScopeKey,
    newWorkoutSummaryScopeKey,
    oldCompletedEntryId: `completed:${badCompletedWorkoutId}`,
    newCompletedEntryId: `completed:${goodCompletedWorkoutId}`,
    oldTrainingPlanPrefix,
    oldTrainingPlanRegex: new RegExp(`^${escapeRegex(oldTrainingPlanPrefix)}`),
  };
}

function processMapping(collections, mapping, config) {
  const result = {
    user_id: mapping.userId,
    bad_wahoo_workout_id: mapping.badWahooWorkoutId,
    good_wahoo_workout_id: mapping.goodWahooWorkoutId,
    bad_completed_workout_id: mapping.badCompletedWorkoutId,
    good_completed_workout_id: mapping.goodCompletedWorkoutId,
    status: "pending",
    blockers: [],
    unsupported_related_state: [],
    repaired: {},
  };

  detectUnsupportedTrainingPlanState(collections, mapping, result, config.maxSamples);
  planCompletedWorkoutRepairs(collections, mapping, result, config.maxSamples);

  if (result.unsupported_related_state.length > 0 && !config.allowPartial) {
    result.blockers.push({
      reason: "unsupported_training_plan_state",
      message:
        "unsupported training-plan state exists for this bad workout id; rerun only after manual cleanup or set MIGRATION_ALLOW_PARTIAL=true",
    });
  }

  if (result.blockers.length > 0) {
    result.status = "blocked";
    return result;
  }

  if (!config.apply) {
    result.status = "dry_run";
    return result;
  }

  applyPlannedRepairs(collections, result);
  result.status = "applied";
  return result;
}

function detectUnsupportedTrainingPlanState(
  collections,
  mapping,
  result,
  maxSamples,
) {
  const unsupportedDetectors = [
    {
      label: "training_plan_generation_operations",
      collection: collections.trainingPlanOperations,
      filter: {
        user_id: mapping.userId,
        $or: [
          { workout_id: mapping.badCompletedWorkoutId },
          { operation_key: mapping.oldTrainingPlanRegex },
        ],
      },
      cursor: () =>
        collections.trainingPlanOperations.find(
          {
            user_id: mapping.userId,
            $or: [
              { workout_id: mapping.badCompletedWorkoutId },
              { operation_key: mapping.oldTrainingPlanRegex },
            ],
          },
          { _id: 0, operation_key: 1, workout_id: 1, status: 1 },
        ),
    },
    {
      label: "training_plan_snapshots",
      collection: collections.trainingPlanSnapshots,
      filter: {
        user_id: mapping.userId,
        $or: [
          { workout_id: mapping.badCompletedWorkoutId },
          { operation_key: mapping.oldTrainingPlanRegex },
        ],
      },
      cursor: () =>
        collections.trainingPlanSnapshots.find(
          {
            user_id: mapping.userId,
            $or: [
              { workout_id: mapping.badCompletedWorkoutId },
              { operation_key: mapping.oldTrainingPlanRegex },
            ],
          },
          { _id: 0, operation_key: 1, workout_id: 1, saved_at_epoch_seconds: 1 },
        ),
    },
    {
      label: "training_plan_projected_days",
      collection: collections.trainingPlanProjectedDays,
      filter: {
        user_id: mapping.userId,
        $or: [
          { workout_id: mapping.badCompletedWorkoutId },
          { operation_key: mapping.oldTrainingPlanRegex },
        ],
      },
      cursor: () =>
        collections.trainingPlanProjectedDays.find(
          {
            user_id: mapping.userId,
            $or: [
              { workout_id: mapping.badCompletedWorkoutId },
              { operation_key: mapping.oldTrainingPlanRegex },
            ],
          },
          { _id: 0, operation_key: 1, workout_id: 1, date: 1 },
        ),
    },
    {
      label: "tasks.training_plan.generate_for_saved_workout",
      collection: collections.tasks,
      filter: {
        user_id: mapping.userId,
        task_type: "training_plan.generate_for_saved_workout",
        $or: [
          { "payload.workout_id": mapping.badCompletedWorkoutId },
          { dedupe_key: mapping.oldTrainingPlanRegex },
        ],
      },
      cursor: () =>
        collections.tasks.find(
          {
            user_id: mapping.userId,
            task_type: "training_plan.generate_for_saved_workout",
            $or: [
              { "payload.workout_id": mapping.badCompletedWorkoutId },
              { dedupe_key: mapping.oldTrainingPlanRegex },
            ],
          },
          {
            _id: 0,
            id: 1,
            status: 1,
            dedupe_key: 1,
            "payload.workout_id": 1,
          },
        ),
    },
  ];

  unsupportedDetectors.forEach((detector) => {
    const documents = detector.cursor().limit(maxSamples + 1).toArray();
    if (documents.length === 0) {
      return;
    }

    result.unsupported_related_state.push({
      label: detector.label,
      count: detector.collection.countDocuments(detector.filter),
      samples: documents.slice(0, maxSamples),
    });
  });
}

function planCompletedWorkoutRepairs(collections, mapping, result, maxSamples) {
  planSingleDocumentRepair({
    collection: collections.completedWorkouts,
    label: "completed_workouts",
    selector: {
      user_id: mapping.userId,
      $or: [
        { completed_workout_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { source_activity_id: { $in: [mapping.badActivityId, mapping.goodActivityId] } },
        { external_id: { $in: [mapping.badActivityId, mapping.goodActivityId] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.completed_workout_id === mapping.badCompletedWorkoutId) {
        updated.completed_workout_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.source_activity_id === mapping.badActivityId) {
        updated.source_activity_id = mapping.goodActivityId;
      }
      if (updated.external_id === mapping.badActivityId) {
        updated.external_id = mapping.goodActivityId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      completed_workout_id:
        document.completed_workout_id === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : document.completed_workout_id,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.wahooFitFiles,
    label: "wahoo_fit_files",
    selector: {
      user_id: mapping.userId,
      $or: [
        { completed_workout_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { wahoo_workout_id: { $in: [mapping.badWahooWorkoutId, mapping.goodWahooWorkoutId] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.completed_workout_id === mapping.badCompletedWorkoutId) {
        updated.completed_workout_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.wahoo_workout_id === mapping.badWahooWorkoutId) {
        updated.wahoo_workout_id = mapping.goodWahooWorkoutId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      completed_workout_id:
        document.completed_workout_id === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : document.completed_workout_id,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.externalObservations,
    label: "external_observations",
    selector: {
      user_id: mapping.userId,
      provider: "wahoo",
      external_object_kind: "completed_workout",
      canonical_entity_kind: "completed_workout",
      $or: [
        { canonical_entity_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { external_id: { $in: [mapping.badActivityId, mapping.goodActivityId] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.canonical_entity_id === mapping.badCompletedWorkoutId) {
        updated.canonical_entity_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.external_id === mapping.badActivityId) {
        updated.external_id = mapping.goodActivityId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      provider: document.provider,
      external_id:
        document.external_id === mapping.badActivityId
          ? mapping.goodActivityId
          : document.external_id,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.externalSyncStates,
    label: "external_sync_states",
    selector: {
      user_id: mapping.userId,
      provider: "wahoo",
      canonical_entity_kind: "completed_workout",
      $or: [
        { canonical_entity_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { external_id: { $in: [mapping.badActivityId, mapping.goodActivityId] } },
        { wahoo_workout_id: { $in: [mapping.badWahooWorkoutId, mapping.goodWahooWorkoutId] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.canonical_entity_id === mapping.badCompletedWorkoutId) {
        updated.canonical_entity_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.external_id === mapping.badActivityId) {
        updated.external_id = mapping.goodActivityId;
      }
      if (updated.wahoo_workout_id === mapping.badWahooWorkoutId) {
        updated.wahoo_workout_id = mapping.goodWahooWorkoutId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      provider: document.provider,
      canonical_entity_kind: document.canonical_entity_kind,
      canonical_entity_id:
        document.canonical_entity_id === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : document.canonical_entity_id,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.workoutSummaries,
    label: "workout_summaries",
    selector: {
      user_id: mapping.userId,
      $or: [
        { workout_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { event_id: { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      const storedWorkoutId = storedWorkoutSummaryId(updated);

      if (storedWorkoutId === mapping.badCompletedWorkoutId) {
        updated.workout_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.event_id === mapping.badCompletedWorkoutId) {
        updated.event_id = mapping.goodCompletedWorkoutId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      workout_id:
        storedWorkoutSummaryId(document) === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : storedWorkoutSummaryId(document),
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.plannedCompletedLinks,
    label: "planned_completed_workout_links",
    selector: {
      user_id: mapping.userId,
      completed_workout_id: {
        $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId],
      },
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.completed_workout_id === mapping.badCompletedWorkoutId) {
        updated.completed_workout_id = mapping.goodCompletedWorkoutId;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      completed_workout_id:
        document.completed_workout_id === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : document.completed_workout_id,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planSingleDocumentRepair({
    collection: collections.tasks,
    label: "tasks.wahoo_fit.enrich",
    selector: {
      user_id: mapping.userId,
      task_type: "wahoo_fit.enrich",
      $or: [
        { "payload.completed_workout_id": { $in: [mapping.badCompletedWorkoutId, mapping.goodCompletedWorkoutId] } },
        { "payload.wahoo_workout_id": { $in: [mapping.badWahooWorkoutId, mapping.goodWahooWorkoutId] } },
        { dedupe_key: { $in: [mapping.oldWahooFitDedupeKey, mapping.newWahooFitDedupeKey] } },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      updated.payload = updated.payload || {};
      if (updated.payload.completed_workout_id === mapping.badCompletedWorkoutId) {
        updated.payload.completed_workout_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.payload.wahoo_workout_id === mapping.badWahooWorkoutId) {
        updated.payload.wahoo_workout_id = mapping.goodWahooWorkoutId;
      }
      if (updated.dedupe_key === mapping.oldWahooFitDedupeKey) {
        updated.dedupe_key = mapping.newWahooFitDedupeKey;
      }
      return updated;
    },
    targetIdentity: (document) => ({
      user_id: document.user_id,
      dedupe_key:
        document.dedupe_key === mapping.oldWahooFitDedupeKey
          ? mapping.newWahooFitDedupeKey
          : document.dedupe_key,
    }),
    identityFilter: (identity) => identity,
    result,
    maxSamples,
  });

  planMultiDocumentRepair({
    collection: collections.llmReplyOperations,
    label: "llm_reply_operations.workout_summary",
    selector: {
      user_id: mapping.userId,
      scope_type: "workout_summary",
      $or: [
        { scope_id: mapping.badCompletedWorkoutId },
        { cache_scope_key: mapping.oldWorkoutSummaryScopeKey },
      ],
    },
    mutate: (document, addBlocker) => {
      const updated = cloneDocument(document);
      if (updated.scope_id === mapping.badCompletedWorkoutId) {
        updated.scope_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.cache_scope_key === mapping.oldWorkoutSummaryScopeKey) {
        updated.cache_scope_key = mapping.newWorkoutSummaryScopeKey;
      }
      if (!updated.user_message_id) {
        addBlocker("matched workout summary reply operation is missing user_message_id", document);
      }
      return updated;
    },
    conflictFilter: (document) => ({
      user_id: document.user_id,
      scope_type: document.scope_type,
      scope_id:
        document.scope_id === mapping.badCompletedWorkoutId
          ? mapping.goodCompletedWorkoutId
          : document.scope_id,
      user_message_id: document.user_message_id,
    }),
    result,
    maxSamples,
  });

  planMultiDocumentRepair({
    collection: collections.tasks,
    label: "tasks.workout_summary.coach_reply",
    selector: {
      user_id: mapping.userId,
      task_type: "workout_summary.coach_reply",
      "payload.workout_id": mapping.badCompletedWorkoutId,
    },
    mutate: (document, addBlocker) => {
      const updated = cloneDocument(document);
      const payload = updated.payload || {};
      if (!payload.user_message_id) {
        addBlocker(
          "matched workout summary coach reply task is missing payload.user_message_id",
          document,
        );
        return updated;
      }

      payload.workout_id = mapping.goodCompletedWorkoutId;
      updated.payload = payload;
      updated.dedupe_key = workoutSummaryCoachReplyDedupeKey(
        updated.user_id,
        mapping.goodCompletedWorkoutId,
        payload.user_message_id,
      );
      return updated;
    },
    conflictFilter: (document) => ({
      user_id: document.user_id,
      dedupe_key: workoutSummaryCoachReplyDedupeKey(
        document.user_id,
        mapping.goodCompletedWorkoutId,
        document.payload.user_message_id,
      ),
    }),
    result,
    maxSamples,
  });

  planMultiDocumentRepair({
    collection: collections.calendarEntryViews,
    label: "calendar_entry_views",
    selector: {
      user_id: mapping.userId,
      $or: [
        { completed_workout_id: mapping.badCompletedWorkoutId },
        { entry_id: mapping.oldCompletedEntryId },
      ],
    },
    mutate: (document) => {
      const updated = cloneDocument(document);
      if (updated.completed_workout_id === mapping.badCompletedWorkoutId) {
        updated.completed_workout_id = mapping.goodCompletedWorkoutId;
      }
      if (updated.entry_id === mapping.oldCompletedEntryId) {
        updated.entry_id = mapping.newCompletedEntryId;
      }
      return updated;
    },
    conflictFilter: (document) => {
      if (document.entry_id !== mapping.oldCompletedEntryId) {
        return null;
      }

      return {
        user_id: document.user_id,
        entry_id: mapping.newCompletedEntryId,
      };
    },
    result,
    maxSamples,
  });
}

function planSingleDocumentRepair({
  collection,
  label,
  selector,
  mutate,
  targetIdentity,
  identityFilter,
  result,
  maxSamples,
}) {
  const documents = collection.find(selector).toArray();
  if (documents.length === 0) {
    result.repaired[label] = { matched: 0, planned_updates: 0 };
    return;
  }

  if (documents.length > 1) {
    addBlocker(result, label, "matched more than one candidate document", documents, maxSamples);
    return;
  }

  const document = documents[0];
  const updated = mutate(document);
  const changed = documentsDiffer(document, updated);
  const target = targetIdentity(document);
  const conflict = collection.findOne({
    ...identityFilter(target),
    _id: { $ne: document._id },
  });

  if (conflict) {
    addBlocker(result, label, "target unique identity already exists", [conflict], maxSamples);
    return;
  }

  result.repaired[label] = {
    matched: 1,
    planned_updates: changed ? 1 : 0,
    operations: changed
      ? [
          {
            kind: "replace_one",
            filter: { _id: document._id },
            replacement: updated,
          },
        ]
      : [],
  };
}

function planMultiDocumentRepair({
  collection,
  label,
  selector,
  mutate,
  conflictFilter,
  result,
  maxSamples,
}) {
  const documents = collection.find(selector).toArray();
  const operations = [];
  let blocked = false;

  documents.forEach((document) => {
    const updated = mutate(document, (message, sample) => {
      blocked = true;
      addBlocker(result, label, message, [sample], maxSamples);
    });
    if (blocked) {
      return;
    }

    const changed = documentsDiffer(document, updated);
    if (!changed) {
      return;
    }

    const filter = conflictFilter(document, updated);
    if (filter) {
      const conflict = collection.findOne({
        ...filter,
        _id: { $ne: document._id },
      });
      if (conflict) {
        blocked = true;
        addBlocker(
          result,
          label,
          "target unique identity already exists",
          [conflict],
          maxSamples,
        );
        return;
      }
    }

    operations.push({
      kind: "replace_one",
      filter: { _id: document._id },
      replacement: updated,
    });
  });

  if (blocked) {
    return;
  }

  result.repaired[label] = {
    matched: documents.length,
    planned_updates: operations.length,
    operations,
  };
}

function applyPlannedRepairs(collections, result) {
  Object.entries(result.repaired).forEach(([label, plan]) => {
    if (!plan.operations || plan.operations.length === 0) {
      return;
    }

    const collection = collectionForLabel(collections, label);
    let modifiedCount = 0;

    plan.operations.forEach((operation) => {
      if (operation.kind !== "replace_one") {
        throw new Error(`unsupported repair operation kind: ${operation.kind}`);
      }

      const writeResult = collection.replaceOne(operation.filter, operation.replacement);
      modifiedCount += writeResult.modifiedCount;
    });

    plan.modified = modifiedCount;
    delete plan.operations;
  });
}

function collectionForLabel(collections, label) {
  switch (label) {
    case "completed_workouts":
      return collections.completedWorkouts;
    case "wahoo_fit_files":
      return collections.wahooFitFiles;
    case "external_observations":
      return collections.externalObservations;
    case "external_sync_states":
      return collections.externalSyncStates;
    case "workout_summaries":
      return collections.workoutSummaries;
    case "planned_completed_workout_links":
      return collections.plannedCompletedLinks;
    case "tasks.wahoo_fit.enrich":
    case "tasks.workout_summary.coach_reply":
      return collections.tasks;
    case "llm_reply_operations.workout_summary":
      return collections.llmReplyOperations;
    case "calendar_entry_views":
      return collections.calendarEntryViews;
    default:
      throw new Error(`unknown repair label: ${label}`);
  }
}

function addBlocker(result, label, message, samples, maxSamples) {
  result.blockers.push({
    label,
    message,
    samples: (samples || []).slice(0, maxSamples).map(toPrintableDocument),
  });
}

function buildSummary(results) {
  const summary = {
    mappings: results.length,
    applied: 0,
    dry_run_ready: 0,
    blocked: 0,
    repaired_documents_planned: 0,
    repaired_documents_modified: 0,
    unsupported_mapping_count: 0,
  };

  results.forEach((result) => {
    if (result.status === "applied") {
      summary.applied += 1;
    }
    if (result.status === "dry_run") {
      summary.dry_run_ready += 1;
    }
    if (result.status === "blocked") {
      summary.blocked += 1;
    }
    if (result.unsupported_related_state.length > 0) {
      summary.unsupported_mapping_count += 1;
    }

    Object.values(result.repaired).forEach((plan) => {
      summary.repaired_documents_planned += plan.planned_updates || 0;
      summary.repaired_documents_modified += plan.modified || 0;
    });
  });

  return summary;
}

function cloneDocument(document) {
  return EJSON.parse(EJSON.stringify(document));
}

function documentsDiffer(left, right) {
  return EJSON.stringify(left) !== EJSON.stringify(right);
}

function workoutSummaryCoachReplyDedupeKey(userId, workoutId, userMessageId) {
  return `workout-summary:${userId}:${workoutId}:${userMessageId}`;
}

function storedWorkoutSummaryId(document) {
  return document.workout_id || document.event_id || null;
}

function toPrintableDocument(document) {
  return EJSON.parse(EJSON.stringify(document));
}

function printSection(title) {
  print(`\n=== ${title} ===`);
}

function readEnv(name, fallbackValue) {
  const value = process.env[name];
  return value === undefined || value === "" ? fallbackValue : value;
}

function readPositiveInt(name, fallbackValue) {
  const raw = readEnv(name, null);
  if (raw === null) {
    return fallbackValue;
  }

  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer, got: ${raw}`);
  }

  return parsed;
}

function readBool(name, fallbackValue) {
  const raw = readEnv(name, null);
  if (raw === null) {
    return fallbackValue;
  }

  const normalized = raw.toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }

  throw new Error(
    `${name} must be one of: 1,true,yes,on,0,false,no,off; got: ${raw}`,
  );
}

function expectString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${label} must be a non-empty string`);
  }

  return value.trim();
}

function expectInteger(value, label) {
  if (!Number.isInteger(value)) {
    throw new Error(`${label} must be an integer, got: ${tojson(value)}`);
  }

  return value;
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
