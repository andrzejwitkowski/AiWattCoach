/*
Usage:

  Run from the repository root:
    MONGODB_DATABASE=aiwatt mongosh "$MONGODB_URI" --file scripts/mongo-backfill-readable-dates.js

Optional environment variables:
  MONGODB_DATABASE  Optional. Uses the current db when omitted.

Notes:
  - This is the main entrypoint for backfilling readable BSON DateTime mirror fields.
  - The actual audited migration logic lives in:
      docs/migrations/2026-04-29-readable-mongo-dates-backfill.mongodb.js
  - The underlying script is idempotent: safe to run multiple times.
  - It backfills only collections that currently have both legacy epoch fields and runtime
    `*_at` DateTime mirror fields in the Mongo adapters.
*/

(function runReadableDatesBackfill() {
  const databaseName = process.env.MONGODB_DATABASE;

  if (databaseName) {
    db = db.getSiblingDB(databaseName);
  }

  print("Running readable-date backfill entrypoint for database: " + db.getName());
  load("docs/migrations/2026-04-29-readable-mongo-dates-backfill.mongodb.js");
})();
