#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

# Run the test and capture stdout to extract the artifact name.
echo "Running real_ai_vs_real_ai test..."
TEST_OUTPUT=$(cd "$REPO_ROOT/server" && cargo test real_ai_vs_real_ai -- --nocapture 2>&1)
echo "$TEST_OUTPUT"

ARTIFACT=$(echo "$TEST_OUTPUT" | grep -o 'REPLAY_ARTIFACT=[^ ]*' | head -1 | cut -d= -f2)
if [ -z "$ARTIFACT" ]; then
    echo "ERROR: could not find REPLAY_ARTIFACT in test output" >&2
    exit 1
fi
echo "Artifact: $ARTIFACT"

# Start the server if not already listening on :8080.
if ! lsof -ti :8080 >/dev/null 2>&1; then
    echo "Starting server..."
    cd "$REPO_ROOT/server"
    cargo run &
    SERVER_PID=$!
    echo "Server PID: $SERVER_PID"
    # Wait until it accepts connections.
    for i in $(seq 1 30); do
        if lsof -ti :8080 >/dev/null 2>&1; then
            break
        fi
        sleep 1
    done
fi

open "http://localhost:8080/dev/selfplay?replay=${ARTIFACT}"
