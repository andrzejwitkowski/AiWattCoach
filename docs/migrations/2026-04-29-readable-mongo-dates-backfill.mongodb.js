// AiWattCoach Mongo readable dates migration
// Run in Studio 3T IntelliShell while connected to the aiwatt database.
// Idempotent: safe to run multiple times.
// This script does not remove collections and does not remove *_epoch_seconds fields,
// because the application still uses dual-read / dual-write timestamp storage.

print("Start readable BSON Date migration for database: " + db.getName())

const numericTypes = ["int", "long", "double", "decimal"]

function missingOrNullFilter(path) {
  return {
    $or: [
      { [path]: { $exists: false } },
      { [path]: null },
    ],
  }
}

function numericFilter(path) {
  return { [path]: { $type: numericTypes } }
}

function backfillRootDate(collectionName, epochPath, datePath) {
  const filter = {
    $and: [
      numericFilter(epochPath),
      missingOrNullFilter(datePath),
    ],
  }

  const setSpec = {}
  setSpec[datePath] = { $toDate: { $multiply: ["$" + epochPath, 1000] } }

  const result = db.getCollection(collectionName).updateMany(filter, [
    { $set: setSpec },
  ])

  print(
    "Backfill " +
      collectionName +
      "." +
      datePath +
      " from " +
      epochPath +
      ": matched=" +
      result.matchedCount +
      ", modified=" +
      result.modifiedCount
  )
}

function backfillArrayDate(collectionName, arrayPath, epochField, dateField) {
  const filter = {
    [arrayPath]: {
      $elemMatch: {
        [epochField]: { $type: numericTypes },
        $or: [
          { [dateField]: { $exists: false } },
          { [dateField]: null },
        ],
      },
    },
  }

  const setSpec = {}
  setSpec[arrayPath] = {
    $map: {
      input: "$" + arrayPath,
      as: "item",
      in: {
        $cond: [
          {
            $and: [
              { $in: [{ $type: "$$item." + epochField }, numericTypes] },
              {
                $or: [
                  { $eq: [{ $type: "$$item." + dateField }, "missing"] },
                  { $eq: ["$$item." + dateField, null] },
                ],
              },
            ],
          },
          {
            $mergeObjects: [
              "$$item",
              {
                [dateField]: {
                  $toDate: {
                    $multiply: ["$$item." + epochField, 1000],
                  },
                },
              },
            ],
          },
          "$$item",
        ],
      },
    },
  }

  const result = db.getCollection(collectionName).updateMany(filter, [
    { $set: setSpec },
  ])

  print(
    "Backfill " +
      collectionName +
      "." +
      arrayPath +
      "[]." +
      dateField +
      " from " +
      epochField +
      ": matched=" +
      result.matchedCount +
      ", modified=" +
      result.modifiedCount
  )
}

[
  ["user_settings", "created_at_epoch_seconds", "created_at"],
  ["user_settings", "updated_at_epoch_seconds", "updated_at"],
  ["user_settings", "intervals.updated_at_epoch_seconds", "intervals.updated_at"],
  ["user_settings", "wahoo.expires_at_epoch_seconds", "wahoo.expires_at"],
  ["user_settings", "wahoo.updated_at_epoch_seconds", "wahoo.updated_at"],
  ["user_settings", "cycling.last_zone_update_epoch_seconds", "cycling.last_zone_update_at"],

  ["workout_summaries", "saved_at_epoch_seconds", "saved_at"],
  ["workout_summaries", "workout_recap_generated_at_epoch_seconds", "workout_recap_generated_at"],
  ["workout_summaries", "created_at_epoch_seconds", "created_at"],
  ["workout_summaries", "updated_at_epoch_seconds", "updated_at"],

  ["athlete_summary", "generated_at_epoch_seconds", "generated_at"],
  ["athlete_summary", "created_at_epoch_seconds", "created_at"],
  ["athlete_summary", "updated_at_epoch_seconds", "updated_at"],

  ["planned_workout_syncs", "created_at_epoch_seconds", "created_at"],
  ["planned_workout_syncs", "updated_at_epoch_seconds", "updated_at"],
  ["planned_workout_syncs", "last_synced_at_epoch_seconds", "last_synced_at"],

  ["planned_workout_wahoo_syncs", "created_at_epoch_seconds", "created_at"],
  ["planned_workout_wahoo_syncs", "updated_at_epoch_seconds", "updated_at"],
  ["planned_workout_wahoo_syncs", "last_synced_at_epoch_seconds", "last_synced_at"],

  ["planned_completed_workout_links", "matched_at_epoch_seconds", "matched_at"],

  ["external_observations", "observed_at_epoch_seconds", "observed_at"],

  ["external_sync_states", "last_synced_at_epoch_seconds", "last_synced_at"],
  ["external_sync_states", "last_seen_remote_at_epoch_seconds", "last_seen_remote_at"],

  ["wahoo_fit_files", "downloaded_at_epoch_seconds", "downloaded_at"],
  ["wahoo_fit_files", "stored_at_epoch_seconds", "stored_at"],
  ["wahoo_fit_files", "parsed_at_epoch_seconds", "parsed_at"],
  ["wahoo_fit_files", "enriched_at_epoch_seconds", "enriched_at"],
  ["wahoo_fit_files", "updated_at_epoch_seconds", "updated_at"],

  ["training_plan_generation_operations", "saved_at_epoch_seconds", "saved_at"],
  ["training_plan_generation_operations", "workout_recap_generated_at_epoch_seconds", "workout_recap_generated_at"],
  ["training_plan_generation_operations", "projection_persisted_at_epoch_seconds", "projection_persisted_at"],
  ["training_plan_generation_operations", "started_at_epoch_seconds", "started_at"],
  ["training_plan_generation_operations", "last_attempt_at_epoch_seconds", "last_attempt_at"],
  ["training_plan_generation_operations", "created_at_epoch_seconds", "created_at"],
  ["training_plan_generation_operations", "updated_at_epoch_seconds", "updated_at"],

  ["athlete_summary_generation_operations", "started_at_epoch_seconds", "started_at"],
  ["athlete_summary_generation_operations", "last_attempt_at_epoch_seconds", "last_attempt_at"],
  ["athlete_summary_generation_operations", "created_at_epoch_seconds", "created_at"],
  ["athlete_summary_generation_operations", "updated_at_epoch_seconds", "updated_at"],

  ["coach_reply_operations", "started_at_epoch_seconds", "started_at"],
  ["coach_reply_operations", "last_attempt_at_epoch_seconds", "last_attempt_at"],
  ["coach_reply_operations", "created_at_epoch_seconds", "created_at"],
  ["coach_reply_operations", "updated_at_epoch_seconds", "updated_at"],

  ["provider_poll_states", "next_due_at_epoch_seconds", "next_due_at"],
  ["provider_poll_states", "last_attempted_at_epoch_seconds", "last_attempted_at"],
  ["provider_poll_states", "last_successful_at_epoch_seconds", "last_successful_at"],
  ["provider_poll_states", "backoff_until_epoch_seconds", "backoff_until_at"],

  ["task_workers", "last_heartbeat_at_epoch_seconds", "last_heartbeat_at"],

  ["tasks", "next_attempt_at_epoch_seconds", "next_attempt_at"],
  ["tasks", "lease_expires_at_epoch_seconds", "lease_expires_at"],
  ["tasks", "last_heartbeat_at_epoch_seconds", "last_heartbeat_at"],
  ["tasks", "timed_out_at_epoch_seconds", "timed_out_at"],
  ["tasks", "created_at_epoch_seconds", "created_at"],
  ["tasks", "updated_at_epoch_seconds", "updated_at"],
  ["tasks", "started_at_epoch_seconds", "started_at"],
  ["tasks", "finished_at_epoch_seconds", "finished_at"],
].forEach(([collectionName, epochPath, datePath]) => {
  backfillRootDate(collectionName, epochPath, datePath)
})

backfillArrayDate("workout_summaries", "messages", "created_at_epoch_seconds", "created_at")
backfillArrayDate("training_plan_generation_operations", "attempts", "recorded_at_epoch_seconds", "recorded_at")

print("No orphaned collections were detected for drop().")
print("No orphaned fields were detected for $unset. *_epoch_seconds fields remain because the app still uses them.")
print("Readable BSON Date migration finished.")
