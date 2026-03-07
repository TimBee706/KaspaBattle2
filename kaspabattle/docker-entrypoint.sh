#!/usr/bin/env bash
# docker-entrypoint.sh — KaspaBattle API startup
# Waits for Postgres, then starts battle-api

set -e

echo "⏳ Waiting for PostgreSQL at $DATABASE_URL ..."
until pg_isready -d "$DATABASE_URL" -q 2>/dev/null; do
  sleep 1
done
echo "✅ PostgreSQL is ready"

echo "🚀 Starting battle-api..."
exec ./battle-api
