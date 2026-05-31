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
# bad Build coord being caught, not silently wrapped) and booted on a private free port. The
# runner owns that server process and tears it down on exit.
#
# Usage:
#   tests/run-all.sh                 # everything (silent unless failing)
#   tests/run-all.sh -v              # verbose: print headers and passes
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
PORT="${PORT:-}"
RUN_RUST=1
RUN_CLIENT=1
VERBOSE=0
for arg in "$@"; do
  case "$arg" in
    --no-rust)   RUN_RUST=0 ;;
    --no-client) RUN_CLIENT=0 ;;
    --port) echo "use --port=N or PORT=N" >&2; exit 2 ;;
    --port=*) PORT="${arg#*=}" ;;
    -v|--verbose) VERBOSE=1 ;;
    -h|--help) sed -n '2,27p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

echo "running all tests silently, will take a while, patience"

if ! command -v node >/dev/null 2>&1; then
  echo "node not found on PATH — the API suites need Node >= 22 (built-in WebSocket)." >&2
  exit 2
fi
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
if [ "$NODE_MAJOR" -lt 22 ]; then
  info "Node $NODE_MAJOR detected; the API suites need >= 22 for the global WebSocket. Continuing anyway."
fi

alloc_port() {
  node -e 'const net = require("node:net"); const s = net.createServer(); s.listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });'
}

if [ -z "$PORT" ]; then
  PORT="$(alloc_port)"
fi

HEALTH_URL="http://127.0.0.1:${PORT}/"
export RTS_WS="ws://127.0.0.1:${PORT}/ws"   # consumed by the Node API suites
export RTS_URL="http://127.0.0.1:${PORT}/"  # consumed by client_smoke.mjs

# --- Output helpers -------------------------------------------------------------------------
if [ -t 1 ]; then BOLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; RST=$'\033[0m'
else BOLD=""; RED=""; GRN=""; YEL=""; RST=""; fi
hdr()  { [ "$VERBOSE" = "1" ] && printf '\n%s== %s ==%s\n' "$BOLD" "$1" "$RST"; }
info() { [ "$VERBOSE" = "1" ] && printf '%s\n' "$1"; }
warn() { printf '%s! %s%s\n' "$YEL" "$1" "$RST"; }

FAILED=()   # human-readable names of suites that failed
SKIPPED=()  # suites we deliberately did not run

# Run a suite, record pass/fail. Args: <name> <command...>
run_suite() {
  local name="$1"; shift
  local logf
  logf="$(mktemp -t rts-suite.XXXXXX)"
  [ "$VERBOSE" = "1" ] && hdr "$name"
  if "$@" >"$logf" 2>&1; then
    rm -f "$logf"
    if [ "$VERBOSE" = "1" ]; then
      info "${GRN}PASS${RST} $name"
    fi
  else
    local rc=$?
    warn "FAIL $name (exit $rc)"
    cat "$logf"
    rm -f "$logf"
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
    if [ "$VERBOSE" = "1" ]; then
      info "stopped server (pid $SERVER_PID)"
    fi
  fi
  [ -n "$SERVER_LOG" ] && rm -f "$SERVER_LOG" 2>/dev/null
}
trap cleanup EXIT INT TERM

boot_server() {
  [ "$VERBOSE" = "1" ] && hdr "Build server (debug)"
  local build_log
  build_log="$(mktemp -t rts-build.XXXXXX)"
  if ! cargo build --manifest-path "$SERVER_DIR/Cargo.toml" >"$build_log" 2>&1; then
    warn "server build failed"
    cat "$build_log"
    rm -f "$build_log"
    FAILED+=("server build")
    return 1
  fi
  rm -f "$build_log"
  local bin="$SERVER_DIR/target/debug/rts-server"
  if [ ! -x "$bin" ]; then
    warn "server binary not found at $bin"
    FAILED+=("server build")
    return 1
  fi

  [ "$VERBOSE" = "1" ] && hdr "Boot server on :$PORT"
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
  if [ "$VERBOSE" = "1" ]; then
    info "server healthy (pid $SERVER_PID) at $HEALTH_URL"
  fi
}

# Parallel background runner — writes pass/fail result to a temp file.
# Usage: run_suite_bg <name> <command...>
# Caller must call collect_bg_results afterwards.
BG_PIDS=()
BG_NAMES=()
BG_RESULT_FILES=()

run_suite_bg() {
  local name="$1"; shift
  local logf resultf
  logf="$(mktemp -t rts-suite.XXXXXX)"
  resultf="$(mktemp -t rts-result.XXXXXX)"
  [ "$VERBOSE" = "1" ] && hdr "$name (bg)"
  (
    if "$@" >"$logf" 2>&1; then
      echo ok >"$resultf"
    else
      echo fail >"$resultf"
    fi
    echo "$logf" >>"$resultf"   # second line = log path
  ) &
  BG_PIDS+=($!)
  BG_NAMES+=("$name")
  BG_RESULT_FILES+=("$resultf")
}

collect_bg_results() {
  local i
  for i in "${!BG_PIDS[@]}"; do
    wait "${BG_PIDS[$i]}" 2>/dev/null || true
    local resultf="${BG_RESULT_FILES[$i]}"
    local name="${BG_NAMES[$i]}"
    local status logf
    status="$(head -1 "$resultf" 2>/dev/null)"
    logf="$(sed -n '2p' "$resultf" 2>/dev/null)"
    if [ "$status" = "ok" ]; then
      [ -n "$logf" ] && rm -f "$logf"
      rm -f "$resultf"
      [ "$VERBOSE" = "1" ] && info "${GRN}PASS${RST} $name"
    else
      warn "FAIL $name"
      [ -n "$logf" ] && { cat "$logf"; rm -f "$logf"; }
      rm -f "$resultf"
      FAILED+=("$name")
    fi
  done
  BG_PIDS=(); BG_NAMES=(); BG_RESULT_FILES=()
}

# --- 1. Build server first (both cargo build and cargo test share the target dir and
#        serialize via cargo's file lock, so we build once, then run both in parallel
#        — cargo test reuses the already-compiled artifacts and mostly just links+runs).
if is_up; then
  info "reusing server already listening on :$PORT (will not stop it)"
  SERVER_HEALTHY=1
  # No build needed; kick off cargo test immediately if requested.
  if [ "$RUN_RUST" = "1" ]; then
    run_suite_bg "Rust scripted tests (cargo test)" \
      cargo test --manifest-path "$SERVER_DIR/Cargo.toml"
  else
    SKIPPED+=("Rust scripted tests (--no-rust)")
  fi
else
  # Build the server binary first (blocks until done).
  if boot_server; then SERVER_HEALTHY=1; else SERVER_HEALTHY=0; fi
  # Now artifacts are compiled; cargo test can reuse them with minimal recompilation.
  if [ "$RUN_RUST" = "1" ]; then
    run_suite_bg "Rust scripted tests (cargo test)" \
      cargo test --manifest-path "$SERVER_DIR/Cargo.toml"
  else
    SKIPPED+=("Rust scripted tests (--no-rust)")
  fi
fi

# --- 2/3. Node suites + client smoke (all parallelised) ------------------------------------
if [ "${SERVER_HEALTHY:-0}" = "1" ]; then
  run_suite_bg "API: server_integration" node "$SCRIPT_DIR/server_integration.mjs"
  run_suite_bg "API: regression"         node "$SCRIPT_DIR/regression.mjs"
  run_suite_bg "API: ai_integration"     node "$SCRIPT_DIR/ai_integration.mjs"

  if [ "$RUN_CLIENT" = "1" ]; then
    CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
    have_puppeteer=0
    [ -d "$SCRIPT_DIR/node_modules/puppeteer-core" ] && have_puppeteer=1
    if [ "$have_puppeteer" = "1" ] && [ -x "$CHROME" ]; then
      CHROME="$CHROME" run_suite_bg "Client smoke (headless Chrome)" node "$SCRIPT_DIR/client_smoke.mjs"
    elif [ "$have_puppeteer" != "1" ]; then
      info "skipping client smoke: puppeteer-core not installed (cd tests && npm install)"
      SKIPPED+=("Client smoke (no puppeteer-core)")
    else
      info "skipping client smoke: no Chrome at \$CHROME ($CHROME)"
      SKIPPED+=("Client smoke (no Chrome)")
    fi
  else
    SKIPPED+=("Client smoke (--no-client)")
  fi
else
  warn "server not healthy — skipping all live-server suites"
  SKIPPED+=("API + client suites (server unavailable)")
fi

collect_bg_results

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
