#!/usr/bin/env bash
# Generate a sample .mindtask.json fixture for testing.
# Usage: ./generate.sh [path-to-mindtask-binary]
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$DIR/../.." && pwd)"

if [ $# -ge 1 ]; then
    MINDTASK="$1"
else
    cargo build --quiet --manifest-path "$REPO/Cargo.toml"
    MINDTASK="$REPO/target/debug/mindtask"
fi
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

$MINDTASK init --timezone "America/New_York"

# Concept tree
$MINDTASK concept add "Backend" --description "Core server-side services"
$MINDTASK concept add "API" --parent 1 --description "HTTP routing and request handlers"
$MINDTASK concept add "Database" --parent 1 --description "Persistence and migrations"
$MINDTASK concept add "Frontend"
$MINDTASK concept add "Dashboard" --parent 4 --description "User-facing metrics dashboard"

# Tasks
$MINDTASK task add "Design API" --duration 2 --due "2025-04-01T09:00"
$MINDTASK task add "Build API" --duration 5 --due "2025-04-08T17:00"
$MINDTASK task add "Write migrations" --duration 1 --due "2025-04-02T12:00"
$MINDTASK task add "Build dashboard" --duration 3 --due "2025-04-10T17:00"
$MINDTASK task add "Code review"
$MINDTASK task add "Deploy"

# Dependencies
$MINDTASK depend add 2 1   # Build API depends on Design API
$MINDTASK depend add 4 2   # Build dashboard depends on Build API
$MINDTASK depend add 5 2   # Code review depends on Build API
$MINDTASK depend add 6 4   # Deploy depends on Build dashboard
$MINDTASK depend add 6 5   # Deploy depends on Code review

# Links (task -> concept)
$MINDTASK link 1 2   # Design API -> API
$MINDTASK link 2 2   # Build API -> API
$MINDTASK link 3 3   # Write migrations -> Database
$MINDTASK link 4 5   # Build dashboard -> Dashboard

# Task states
$MINDTASK task state 1 done
$MINDTASK task state 2 in_progress
$MINDTASK task state 3 done

cp .mindtask.json "$DIR/sample.json"
echo "Wrote $DIR/sample.json"
