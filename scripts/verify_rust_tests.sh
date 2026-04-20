#!/usr/bin/env sh

set -eu

cargo test --lib

for target in \
  athlete_summary_service \
  auth_rest \
  calendar_entry_views_mongo \
  canonical_roots_mongo \
  dashboard_rest \
  external_sync_mongo \
  health_check \
  identity_domain \
  identity_service \
  intervals_adapters \
  intervals_pest_parser_poc \
  intervals_planned_workout \
  intervals_rest \
  intervals_service \
  intervals_workout_analysis \
  llm_adapters \
  llm_rest \
  logs_rest \
  main_runtime \
  races_mongo \
  settings \
  settings_rest \
  task_scheduler \
  telemetry_setup \
  training_load_mongo \
  training_plan_mongo \
  training_plan_service \
  workout_summary_mongo \
  workout_summary_rest \
  workout_summary_service
do
  cargo test --test "$target"
done

cargo test --bin aiwattcoach
