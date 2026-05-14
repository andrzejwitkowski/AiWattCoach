/*
Usage:

  MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-detect-planned-workout-split-brains.js

Optional environment variables:
  MONGODB_DATABASE          Optional. Uses the current db when omitted.
  DETECT_USER_ID            Optional. Limit detection to one user.
  DETECT_MAX_SAMPLES        Optional. Default: 25.
  DETECT_FAIL_ON_FINDINGS   Optional. true/false. Default: false.
  APPLY                     Optional. true/false. Default: false.

This is a read-only detector for the split-brain pattern where a same-day planned workout
was superseded, but stale source rows still point at the older planned workout id.

When APPLY=true, the script repairs only stale-owner-only findings by deleting stale
Intervals planned-workout sync owner rows on superseded same-day planned ids.
*/

(function detectPlannedWorkoutSplitBrains() {
  const config = readConfig();
  const database = resolveDatabase(config.databaseName);
  const collections = {
    projectedDays: database.getCollection("training_plan_projected_days"),
    externalSyncStates: database.getCollection("external_sync_states"),
    completedWorkouts: database.getCollection("completed_workouts"),
    plannedCompletedLinks: database.getCollection("planned_completed_workout_links"),
    calendarEntryViews: database.getCollection("calendar_entry_views"),
  };

  const initial = detectFindings(collections, config.userId);

  printSection("Configuration");
  printjson({
    database: database.getName(),
    userId: config.userId,
    candidateDays: initial.candidateDays.length,
    maxSamples: config.maxSamples,
    failOnFindings: config.failOnFindings,
    apply: config.apply,
  });

  printDetectionReport("Summary", initial.findings, initial.summary, config.maxSamples);

  let final = initial;
  if (config.apply) {
    const repairResult = applyRepairs(collections, initial.findings);
    printSection("Repair Summary");
    printjson(repairResult.summary);
    if (repairResult.skipped.length > 0) {
      printSection("Repair Skips");
      print(
        EJSON.stringify(
          repairResult.skipped.slice(0, config.maxSamples).map(toPrintableFinding),
          null,
          2,
        ),
      );
    }

    final = detectFindings(collections, config.userId);
    printDetectionReport("Post-Repair Summary", final.findings, final.summary, config.maxSamples);
  }

  if (config.failOnFindings && final.findings.length > 0) {
    print("Split-brain findings detected.");
    quit(1);
  }

  print(final.findings.length > 0 ? "Split-brain findings detected." : "No split-brain findings detected.");
})();

function readConfig() {
  return {
    databaseName: readEnv("MONGODB_DATABASE", null),
    userId: readEnv("DETECT_USER_ID", null),
    maxSamples: readPositiveInt("DETECT_MAX_SAMPLES", 25),
    failOnFindings: readBool("DETECT_FAIL_ON_FINDINGS", false),
    apply: readBool("APPLY", false),
  };
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

function resolveDatabase(databaseName) {
  return databaseName ? db.getSiblingDB(databaseName) : db;
}

function loadCandidateDays(projectedDays, userId) {
  const filter = userId ? { user_id: userId } : {};
  const projection = {
    _id: 0,
    user_id: 1,
    date: 1,
    operation_key: 1,
    superseded_at_epoch_seconds: 1,
  };
  const grouped = new Map();
  const cursor = projectedDays.find(filter, projection).sort({ user_id: 1, date: 1, operation_key: 1 });

  while (cursor.hasNext()) {
    recordProjectedDay(grouped, cursor.next());
  }

  return Array.from(grouped.values()).filter(
    (entry) => entry.active_planned_ids.length > 0 && entry.stale_planned_ids.length > 0,
  );
}

function loadCandidateDay(projectedDays, userId, date) {
  const grouped = new Map();
  const cursor = projectedDays
    .find(
      { user_id: userId, date },
      {
        _id: 0,
        user_id: 1,
        date: 1,
        operation_key: 1,
        superseded_at_epoch_seconds: 1,
      },
    )
    .sort({ user_id: 1, date: 1, operation_key: 1 });

  while (cursor.hasNext()) {
    recordProjectedDay(grouped, cursor.next());
  }

  const candidate = grouped.get(`${userId}|${date}`);
  if (!candidate) {
    return null;
  }

  return candidate.active_planned_ids.length > 0 && candidate.stale_planned_ids.length > 0
    ? candidate
    : null;
}

function recordProjectedDay(grouped, day) {
  const key = `${day.user_id}|${day.date}`;
  let entry = grouped.get(key);
  if (!entry) {
    entry = {
      user_id: day.user_id,
      date: day.date,
      active_planned_ids: [],
      stale_planned_ids: [],
      active_operation_keys: [],
      stale_operation_keys: [],
    };
    grouped.set(key, entry);
  }

  const plannedWorkoutId = `${day.operation_key}:${day.date}`;
  if (day.superseded_at_epoch_seconds === null || day.superseded_at_epoch_seconds === undefined) {
    entry.active_planned_ids.push(plannedWorkoutId);
    entry.active_operation_keys.push(day.operation_key);
  } else {
    entry.stale_planned_ids.push(plannedWorkoutId);
    entry.stale_operation_keys.push(day.operation_key);
  }
}

function detectFindings(collections, userId) {
  const candidateDays = loadCandidateDays(collections.projectedDays, userId);
  const findings = candidateDays
    .map((candidate) => detectCandidateIssues(collections, candidate))
    .filter((candidate) => candidate.issue_kinds.length > 0);

  return {
    candidateDays,
    findings,
    summary: buildSummary(candidateDays, findings),
  };
}

function detectCandidateIssues(collections, candidate) {
  const staleIntervalsSyncOwners = collections.externalSyncStates
    .find(
      {
        user_id: candidate.user_id,
        provider: "intervals",
        canonical_entity_kind: "planned_workout",
        canonical_entity_id: { $in: candidate.stale_planned_ids },
        external_id: { $type: "string" },
      },
      {
        _id: 0,
        canonical_entity_id: 1,
        external_id: 1,
        sync_status: 1,
      },
    )
    .toArray();

  const currentIntervalsSyncRows = collections.externalSyncStates
    .find(
      {
        user_id: candidate.user_id,
        provider: "intervals",
        canonical_entity_kind: "planned_workout",
        canonical_entity_id: { $in: candidate.active_planned_ids },
      },
      {
        _id: 0,
        canonical_entity_id: 1,
        external_id: 1,
        sync_status: 1,
      },
    )
    .toArray();

  const staleCompletedBacklinks = collections.completedWorkouts
    .find(
      {
        user_id: candidate.user_id,
        planned_workout_id: { $in: candidate.stale_planned_ids },
      },
      {
        _id: 0,
        completed_workout_id: 1,
        planned_workout_id: 1,
        start_date_local: 1,
      },
    )
    .toArray();

  const stalePlannedCompletedLinks = collections.plannedCompletedLinks
    .find(
      {
        user_id: candidate.user_id,
        planned_workout_id: { $in: candidate.stale_planned_ids },
      },
      {
        _id: 0,
        planned_workout_id: 1,
        completed_workout_id: 1,
        match_source: 1,
      },
    )
    .toArray();

  const calendarRows = collections.calendarEntryViews
    .find(
      {
        user_id: candidate.user_id,
        date: candidate.date,
      },
      {
        _id: 0,
        entry_id: 1,
        entry_kind: 1,
        planned_workout_id: 1,
        completed_workout_id: 1,
        sync: 1,
      },
    )
    .sort({ entry_id: 1 })
    .toArray();

  const hasCurrentPlannedView = calendarRows.some(
    (row) =>
      row.entry_kind === "planned_workout" &&
      candidate.active_planned_ids.includes(row.planned_workout_id),
  );
  const staleCompletedCalendarRows = calendarRows.filter(
    (row) =>
      row.entry_kind === "completed_workout" &&
      candidate.stale_planned_ids.includes(row.planned_workout_id),
  );
  const duplicateKeyRisk =
    staleIntervalsSyncOwners.length > 0 &&
    currentIntervalsSyncRows.some((row) => row.external_id === null || row.external_id === undefined);

  const issueKinds = [];
  if (staleIntervalsSyncOwners.length > 0) {
    issueKinds.push("stale_intervals_sync_owner");
  }
  if (staleCompletedBacklinks.length > 0) {
    issueKinds.push("stale_completed_backlink");
  }
  if (stalePlannedCompletedLinks.length > 0) {
    issueKinds.push("stale_planned_completed_link");
  }
  if (hasCurrentPlannedView && staleCompletedCalendarRows.length > 0) {
    issueKinds.push("calendar_read_model_split");
  }
  if (duplicateKeyRisk) {
    issueKinds.push("duplicate_key_risk_on_resync");
  }

  return {
    user_id: candidate.user_id,
    date: candidate.date,
    active_planned_ids: candidate.active_planned_ids,
    stale_planned_ids: candidate.stale_planned_ids,
    issue_kinds: issueKinds,
    stale_intervals_sync_owners: staleIntervalsSyncOwners,
    current_intervals_sync_rows: currentIntervalsSyncRows,
    stale_completed_backlinks: staleCompletedBacklinks,
    stale_planned_completed_links: stalePlannedCompletedLinks,
    stale_completed_calendar_rows: staleCompletedCalendarRows,
    calendar_rows: calendarRows,
  };
}

function buildSummary(candidateDays, findings) {
  const issueCounts = {
    stale_intervals_sync_owner: 0,
    stale_completed_backlink: 0,
    stale_planned_completed_link: 0,
    calendar_read_model_split: 0,
    duplicate_key_risk_on_resync: 0,
  };

  findings.forEach((finding) => {
    finding.issue_kinds.forEach((kind) => {
      issueCounts[kind] += 1;
    });
  });

  return {
    candidateDaysScanned: candidateDays.length,
    affectedDays: findings.length,
    affectedUsers: new Set(findings.map((finding) => finding.user_id)).size,
    issueCounts,
  };
}

function printDetectionReport(title, findings, summary, maxSamples) {
  printSection(title);
  printjson(summary);

  printSection(title === "Summary" ? "Samples" : `${title} Samples`);
  print(
    EJSON.stringify(
      findings.slice(0, maxSamples).map(toPrintableFinding),
      null,
      2,
    ),
  );
}

function applyRepairs(collections, findings) {
  const repairable = findings.filter(isRepairableFinding);
  const skipped = findings
    .filter((finding) => !isRepairableFinding(finding))
    .map((finding) => ({
      user_id: finding.user_id,
      date: finding.date,
      status: "skipped",
      reason: "finding_not_repairable",
      issue_kinds: finding.issue_kinds,
    }));
  const outcomes = repairable.map((finding) => applyRepair(collections, finding));
  const applied = outcomes.filter((outcome) => outcome.status === "applied");
  skipped.push(...outcomes.filter((outcome) => outcome.status !== "applied"));

  return {
    applied,
    skipped,
    summary: {
      repairableDays: repairable.length,
      repairedDays: applied.length,
      skippedDays: skipped.length,
      deletedStaleIntervalsSyncOwners: applied.reduce(
        (count, entry) => count + entry.deleted_count,
        0,
      ),
      repairedSamples: applied,
      skippedSamples: skipped,
    },
  };
}

function isRepairableFinding(finding) {
  return (
    finding.issue_kinds.length === 1 &&
    finding.issue_kinds[0] === "stale_intervals_sync_owner" &&
    finding.active_planned_ids.length === 1 &&
    finding.stale_intervals_sync_owners.length > 0
  );
}

function applyRepair(collections, finding) {
  const currentCandidate = loadCandidateDay(
    collections.projectedDays,
    finding.user_id,
    finding.date,
  );
  if (!currentCandidate) {
    return {
      user_id: finding.user_id,
      date: finding.date,
      status: "skipped",
      reason: "candidate_disappeared_before_repair",
    };
  }

  const currentFinding = detectCandidateIssues(collections, currentCandidate);
  if (!isRepairableFinding(currentFinding)) {
    return {
      user_id: finding.user_id,
      date: finding.date,
      status: "skipped",
      reason: "finding_no_longer_repairable",
      issue_kinds: currentFinding.issue_kinds,
    };
  }

  const staleOwnerIds = currentFinding.stale_intervals_sync_owners.map(
    (owner) => owner.canonical_entity_id,
  );
  const deleteResult = collections.externalSyncStates.deleteMany({
    user_id: finding.user_id,
    provider: "intervals",
    canonical_entity_kind: "planned_workout",
    canonical_entity_id: { $in: staleOwnerIds },
    external_id: { $type: "string" },
  });

  if (deleteResult.deletedCount !== staleOwnerIds.length) {
    throw new Error(
      `Expected to delete ${staleOwnerIds.length} stale Intervals sync rows for ${finding.user_id} ${finding.date}, deleted ${deleteResult.deletedCount}`,
    );
  }

  return {
    user_id: finding.user_id,
    date: finding.date,
    status: "applied",
    active_planned_id: currentFinding.active_planned_ids[0],
    deleted_count: deleteResult.deletedCount,
    deleted_stale_planned_ids: staleOwnerIds,
  };
}

function toPrintableFinding(finding) {
  return {
    user_id: finding.user_id,
    date: finding.date,
    issue_kinds: finding.issue_kinds,
    active_planned_ids: finding.active_planned_ids,
    stale_planned_ids: finding.stale_planned_ids,
    stale_intervals_sync_owners: finding.stale_intervals_sync_owners,
    current_intervals_sync_rows: finding.current_intervals_sync_rows,
    stale_completed_backlinks: finding.stale_completed_backlinks,
    stale_planned_completed_links: finding.stale_planned_completed_links,
    stale_completed_calendar_rows: finding.stale_completed_calendar_rows,
  };
}

function printSection(title) {
  print(`\n=== ${title} ===`);
}
