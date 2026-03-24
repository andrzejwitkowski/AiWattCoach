#!/bin/sh

set -eu

mkdir -p /var/log/aiwattcoach

mkfifo /tmp/aiwattcoach.stdout
tee -a /var/log/aiwattcoach/app.log < /tmp/aiwattcoach.stdout &
tee_pid=$!

aiwattcoach > /tmp/aiwattcoach.stdout 2>&1 &
app_pid=$!

forward_signal() {
  kill -TERM "$app_pid" 2>/dev/null || true
}

trap forward_signal TERM INT

wait "$app_pid"
app_status=$?
wait "$tee_pid" || true
rm -f /tmp/aiwattcoach.stdout
exit "$app_status"
