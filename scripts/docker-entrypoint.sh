#!/bin/sh
set -eu

# New hosting volumes can be owned by root. Prepare only this database's files,
# then run the server as the unprivileged application user.
if [ "$(id -u)" = "0" ]; then
  database_file="${DATABASE_PATH:-/app/data/weekend-club.sqlite}"
  database_dir="$(dirname "$database_file")"
  mkdir -p "$database_dir"
  chown appuser:appuser "$database_dir"
  for database_part in "$database_file" "$database_file-wal" "$database_file-shm"; do
    if [ -e "$database_part" ]; then
      chown appuser:appuser "$database_part"
    fi
  done
  exec gosu appuser "$@"
fi

exec "$@"
