#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
P1="rifle_flood_fast"
P2="tech_to_tanks"
TICKS="50000"
SEED="0"
ARTIFACT=""
OPEN_REPLAY=0
CLEAR_REPLAYS=0
KILL_EXISTING=0
LOG_FILE="$(mktemp -t runaimatchup.XXXXXX.log)"
SERVER_PID=""
SERVER_URL=""

usage() {
    cat <<'EOF'
Usage: ./runaimatchup.sh [options]

Options:
  --p1=PROFILE              Player 1 profile (default: rifle_flood_fast)
  --p2=PROFILE              Player 2 profile (default: tech_to_tanks)
  --ticks=N                 Max simulated ticks (default: 50000)
  --seed=N                  Match seed (default: 0)
  --artifact=NAME           Replay artifact name
  --open                    Open the replay after the run
  --clear-replays           Remove old self-play artifacts/failures before running
  --kill-existing-servers   Kill existing rts-server processes before running/opening
  -h, --help                Show this help

Profiles:
  rifle_flood_fast
  rifle_flood_full_saturation
  tech_to_tanks
EOF
}

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

alloc_port() {
    node -e 'const net = require("node:net"); const s = net.createServer(); s.listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });'
}

safe_artifact_name() {
    printf '%s' "$1" | tr -c 'A-Za-z0-9_.-' '_'
}

for arg in "$@"; do
    case "$arg" in
        --p1=*) P1="${arg#*=}" ;;
        --p2=*) P2="${arg#*=}" ;;
        --ticks=*) TICKS="${arg#*=}" ;;
        --seed=*) SEED="${arg#*=}" ;;
        --artifact=*) ARTIFACT="${arg#*=}" ;;
        --open) OPEN_REPLAY=1 ;;
        --clear-replays) CLEAR_REPLAYS=1 ;;
        --kill-existing-servers) KILL_EXISTING=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; usage >&2; exit 2 ;;
    esac
done

if ! command -v rg >/dev/null 2>&1; then
    echo "rg not found on PATH." >&2
    exit 2
fi
if [ "$OPEN_REPLAY" -eq 1 ] && ! command -v node >/dev/null 2>&1; then
    echo "node not found on PATH; --open needs Node to allocate a free port." >&2
    exit 2
fi

if [ "$KILL_EXISTING" -eq 1 ]; then
    pkill -f 'rts-server|target/debug/rts-server|./target/debug/rts-server' 2>/dev/null || true
fi

if [ "$CLEAR_REPLAYS" -eq 1 ]; then
    rm -rf "$REPO_ROOT/server/target/selfplay-artifacts" \
        "$REPO_ROOT/server/target/selfplay-failures"
fi

if [ -z "$ARTIFACT" ]; then
    ARTIFACT="$(safe_artifact_name "profile_${P1}_vs_${P2}_${TICKS}_seed_${SEED}")"
fi

echo "Running $P1 vs $P2 for up to $TICKS ticks (seed $SEED)..."
set +e
(
    cd "$REPO_ROOT/server"
    RTS_MATCHUP_P1="$P1" \
    RTS_MATCHUP_P2="$P2" \
    RTS_MATCHUP_TICKS="$TICKS" \
    RTS_MATCHUP_SEED="$SEED" \
    RTS_MATCHUP_ARTIFACT="$ARTIFACT" \
        cargo test profile_matchup_result_tool -- --ignored --nocapture --test-threads=1
) >"$LOG_FILE" 2>&1
TEST_STATUS=$?
set -e
cat "$LOG_FILE"

SIM_RESULT=$(rg 'SIM_RESULT' "$LOG_FILE" | tail -1 || true)
if [ -z "$SIM_RESULT" ]; then
    echo "ERROR: no SIM_RESULT found in test output" >&2
    exit "$TEST_STATUS"
fi
echo "$SIM_RESULT"

if [ "$TEST_STATUS" -ne 0 ]; then
    echo "Matchup run failed with status $TEST_STATUS." >&2
    exit "$TEST_STATUS"
fi

if [ "$OPEN_REPLAY" -eq 0 ]; then
    echo "Replay artifact: $ARTIFACT"
    exit 0
fi

PORT="$(alloc_port)"
SERVER_URL="http://127.0.0.1:${PORT}"
SERVER_LOG="$(mktemp -t runaimatchup-server.XXXXXX.log)"
echo "Starting server on $SERVER_URL..."
(
    cd "$REPO_ROOT/server"
    RTS_ADDR="127.0.0.1:${PORT}" cargo run >"$SERVER_LOG" 2>&1
) &
SERVER_PID=$!

for _ in $(seq 1 60); do
    if curl -fsS --max-time 1 "$SERVER_URL/" >/dev/null 2>&1; then
        break
    fi
    sleep 0.5
done

if ! curl -fsS --max-time 1 "$SERVER_URL/" >/dev/null 2>&1; then
    echo "ERROR: server did not start; log follows:" >&2
    cat "$SERVER_LOG" >&2
    exit 1
fi

open "${SERVER_URL}/dev/selfplay?replay=${ARTIFACT}"
echo "Replay opened: ${SERVER_URL}/dev/selfplay?replay=${ARTIFACT}"
echo "Server running at ${SERVER_URL} — press Ctrl+C to stop."
wait "$SERVER_PID" || true
