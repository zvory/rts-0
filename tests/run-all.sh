#!/usr/bin/env bash
# Run the whole test suite against a freshly-built, freshly-booted server, then exit
# non-zero if anything failed. This is the canonical "is the build green?" command.
#
# What it runs, in order:
#   1. Rust scripted tests          (cargo test — deterministic, in-process, no server)
#   2. Node API suites              (server_integration, regression, ai_integration)
#   3. Headless client smoke        (client_smoke — only if puppeteer-core + Chrome are present)
#
# The server is built in debug (overflow checks ON — the hardening regression tests rely on a
# bad Build coord being caught, not silently wrapped) and booted on a free-ish port. If a server
# is already answering on the chosen port it is reused (and left running); otherwise this script
# starts one and tears it down on exit.
#
# Usage:
#   tests/run-all.sh                 # everything
#   tests/run-all.sh --no-rust       # skip the cargo test step
#   tests/run-all.sh --no-client     # skip the headless-browser smoke test
#   PORT=8090 tests/run-all.sh       # use a different port
#   CHROME=/path/to/chrome tests/run-all.sh
set -uo pipefail

# --- Layout ---------------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$REPO_ROOT/server"

# --- Options --------------------------------------------------------------------------------
PORT="${PORT:-8081}"
RUN_RUST=1
RUN_CLIENT=1
for arg in "$@"; do
  case "$arg" in
    --no-rust)   RUN_RUST=0 ;;
    --no-client) RUN_CLIENT=0 ;;
    --port) echo "use --port=N or PORT=N" >&2; exit 2 ;;
    --port=*) PORT="${arg#*=}" ;;
    -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

HEALTH_URL="http://127.0.0.1:${PORT}/"
export RTS_WS="ws://127.0.0.1:${PORT}/ws"   # consumed by the Node API suites
export RTS_URL="http://127.0.0.1:${PORT}/"  # consumed by client_smoke.mjs

# --- Output helpers -------------------------------------------------------------------------
if [ -t 1 ]; then BOLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; RST=$'\033[0m'
else BOLD=""; RED=""; GRN=""; YEL=""; RST=""; fi
hdr()  { printf '\n%s== %s ==%s\n' "$BOLD" "$1" "$RST"; }
info() { printf '%s\n' "$1"; }
warn() { printf '%s! %s%s\n' "$YEL" "$1" "$RST"; }

FAILED=()   # human-readable names of suites that failed
SKIPPED=()  # suites we deliberately did not run

# Run a suite, record pass/fail. Args: <name> <command...>
run_suite() {
  local name="$1"; shift
  hdr "$name"
  if "$@"; then
    info "${GRN}PASS${RST} $name"
  else
    warn "FAIL $name (exit $?)"
    FAILED+=("$name")
  fi
}

# --- Server lifecycle -----------------------------------------------------------------------
SERVER_PID=""
SERVER_LOG=""
STARTED_SERVER=0

is_up() { curl -fsS -o /dev/null --max-time 2 "$HEALTH_URL" 2>/dev/null; }

cleanup() {
  if [ "$STARTED_SERVER" = "1" ] && [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null
    wait "$SERVER_PID" 2>/dev/null
    info "stopped server (pid $SERVER_PID)"
  fi
  [ -n "$SERVER_LOG" ] && rm -f "$SERVER_LOG" 2>/dev/null
}
trap cleanup EXIT INT TERM

boot_server() {
  hdr "Build server (debug)"
  if ! cargo build --manifest-path "$SERVER_DIR/Cargo.toml"; then
    warn "server build failed"
    FAILED+=("server build")
    return 1
  fi
  local bin="$SERVER_DIR/target/debug/rts-server"
  if [ ! -x "$bin" ]; then
    warn "server binary not found at $bin"
    FAILED+=("server build")
    return 1
  fi

  hdr "Boot server on :$PORT"
  SERVER_LOG="$(mktemp -t rts-server-log.XXXXXX)"
  RTS_ADDR="0.0.0.0:${PORT}" "$bin" >"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!
  STARTED_SERVER=1

  # Health-check: poll GET / until 200, the server dies, or we time out.
  local deadline=$((SECONDS + 30))
  while ! is_up; do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      warn "server exited during startup; log:"; sed 's/^/  /' "$SERVER_LOG" >&2
      FAILED+=("server boot")
      return 1
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      warn "server did not become healthy within 30s; log:"; sed 's/^/  /' "$SERVER_LOG" >&2
      FAILED+=("server boot")
      return 1
    fi
    sleep 0.3
  done
  info "server healthy (pid $SERVER_PID) at $HEALTH_URL"
}

# --- Preflight ------------------------------------------------------------------------------
if ! command -v node >/dev/null 2>&1; then
  echo "node not found on PATH — the API suites need Node >= 22 (built-in WebSocket)." >&2
  exit 2
fi
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
if [ "$NODE_MAJOR" -lt 22 ]; then
  warn "Node $NODE_MAJOR detected; the API suites need >= 22 for the global WebSocket. Continuing anyway."
fi

# --- 1. Rust scripted tests (no server needed) ---------------------------------------------
if [ "$RUN_RUST" = "1" ]; then
  run_suite "Rust scripted tests (cargo test)" \
    cargo test --manifest-path "$SERVER_DIR/Cargo.toml"
else
  SKIPPED+=("Rust scripted tests (--no-rust)")
fi

# --- 2/3. Anything needing a live server ----------------------------------------------------
if is_up; then
  info "reusing server already listening on :$PORT (will not stop it)"
  SERVER_HEALTHY=1
else
  if boot_server; then SERVER_HEALTHY=1; else SERVER_HEALTHY=0; fi
fi

if [ "${SERVER_HEALTHY:-0}" = "1" ]; then
  run_suite "API: server_integration" node "$SCRIPT_DIR/server_integration.mjs"
  run_suite "API: regression"         node "$SCRIPT_DIR/regression.mjs"
  run_suite "API: ai_integration"     node "$SCRIPT_DIR/ai_integration.mjs"

  if [ "$RUN_CLIENT" = "1" ]; then
    CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
    have_puppeteer=0
    [ -d "$SCRIPT_DIR/node_modules/puppeteer-core" ] && have_puppeteer=1
    if [ "$have_puppeteer" = "1" ] && [ -x "$CHROME" ]; then
      CHROME="$CHROME" run_suite "Client smoke (headless Chrome)" node "$SCRIPT_DIR/client_smoke.mjs"
    elif [ "$have_puppeteer" != "1" ]; then
      warn "skipping client smoke: puppeteer-core not installed (cd tests && npm install)"
      SKIPPED+=("Client smoke (no puppeteer-core)")
    else
      warn "skipping client smoke: no Chrome at \$CHROME ($CHROME)"
      SKIPPED+=("Client smoke (no Chrome)")
    fi
  else
    SKIPPED+=("Client smoke (--no-client)")
  fi
else
  warn "server not healthy — skipping all live-server suites"
  SKIPPED+=("API + client suites (server unavailable)")
fi

# --- Summary --------------------------------------------------------------------------------
hdr "Summary"
for s in "${SKIPPED[@]:-}"; do [ -n "$s" ] && info "  ${YEL}SKIP${RST} $s"; done
if [ "${#FAILED[@]}" -eq 0 ]; then
  info "  ${GRN}ALL SUITES PASSED ✅${RST}"
  exit 0
else
  for f in "${FAILED[@]}"; do info "  ${RED}FAIL${RST} $f"; done
  info "${RED}${#FAILED[@]} suite(s) failed ❌${RST}"
  exit 1
fi
