#!/bin/sh

set -eu

mkdir -p /var/log/aiwattcoach

# Truncate the log file on container start to prevent unbounded growth
: > /var/log/aiwattcoach/app.log

# Remove stale FIFO from a previous crash if present
rm -f /tmp/aiwattcoach.stdout
mkfifo /tmp/aiwattcoach.stdout
tee -a /var/log/aiwattcoach/app.log < /tmp/aiwattcoach.stdout &
tee_pid=$!

aiwattcoach > /tmp/aiwattcoach.stdout 2>&1 &
app_pid=$!

forward_signal() {
  kill -TERM "$app_pid" 2>/dev/null || true
}

trap forward_signal TERM INT

# Wait once and capture the real exit code without aborting on non-zero exit
set +e
wait "$app_pid"
app_status=$?
set -e
wait "$tee_pid" || true
rm -f /tmp/aiwattcoach.stdout
exit "$app_status"
