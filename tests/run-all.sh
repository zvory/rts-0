#!/usr/bin/env bash
# Run the whole test suite against a freshly-built, freshly-booted server, then exit
# non-zero if anything failed. This is the canonical "is the build green?" command.
#
# What it runs, in order:
#   1. Architecture/contract policy (source file sizes + crate boundaries + sim/client/lobby architecture + faction guardrails + test-selector self-check)
#   2. Rust nextest fast scripted tests (deterministic, in-process, no server)
#   3. Rust lint                    (cargo clippy)
#   4. Node API suites              (protocol/UI units, live API batch, then serialized lab_mortar_regression)
#   5. Headless browser suites      (client and Interact smokes, plus opted-in tri-state scenarios; needs Chrome)
#
# The server is built in debug (overflow checks ON — the hardening regression tests rely on a
# bad Build coord being caught, not silently wrapped) and booted on a private free port. The
# runner owns that server process and tears it down on exit.
# The private test server runs with a 5ms tick interval so live-server tests wait on simulated
# game progress instead of real-time 30 Hz wall clock. Normal `cargo run` remains 30 Hz.
#
# Usage:
#   tests/run-all.sh                 # local gate (silent unless failing)
#   tests/run-all.sh --full-ai       # also run long AI self-play/simulation coverage
#   tests/run-all.sh -v              # verbose: print headers and pass lines; timings are always summarized
#   tests/run-all.sh --no-rust       # skip Rust test/lint
#   tests/run-all.sh --no-client     # skip the headless-browser smoke test
#   tests/run-all.sh --only-rust     # run architecture policy + Rust test/lint only
#   tests/run-all.sh --only-rust-checks # run Rust architecture policy + lint, without nextest
#   tests/run-all.sh --only-nextest  # run Rust nextest only (honors RTS_NEXTEST_PARTITION)
#   tests/run-all.sh --only-live-node # run JS contracts + live Node API suites only
#   tests/run-all.sh --only-browser  # run browser suites only
#   tests/run-all.sh --only-browser-scenarios=smoke,phase-0.5  # run an explicit browser shard
#   tests/run-all.sh --with-tri-state-browser  # run latency-sensitive browser tri-state scenarios locally
#   PORT=8090 tests/run-all.sh       # use a different port
#   RTS_MATCH_SEED=123 tests/run-all.sh  # use a different deterministic map seed
#   CARGO_TARGET_DIR=/path/to/target tests/run-all.sh  # override the per-worktree Cargo target dir
#   RTS_SERVER_BIN=/path/to/rts-server tests/run-all.sh --only-live-node  # reuse a prebuilt server
#   RTS_NODE_DEPS_CACHE_DIR=/tmp/rts-node-deps tests/run-all.sh
#   RTS_RUN_TRI_STATE_BROWSER=1 tests/run-all.sh  # env-form local opt-in for tri-state browser scenarios
#   RTS_RUN_WASM_TRI_STATE=0 tests/run-all.sh     # skip WASM-backed tri-state groups even when assets exist
#   RTS_NEXTEST_PARTITION=slice:1/2 tests/run-all.sh --only-nextest  # run one nextest shard
#   CHROME=/path/to/chrome tests/run-all.sh
set -uo pipefail

# --- Layout ---------------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$REPO_ROOT/server"

# Cargo normally writes build artifacts under each worktree's server/target directory. Local gate
# runs use a deterministic target dir under /tmp per worktree so fresh worktrees do not clutter the
# checkout and parallel agents do not share final binaries or test artifacts. Callers can still
# override CARGO_TARGET_DIR.
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  CARGO_TARGET_DIR="$("$REPO_ROOT/scripts/cargo-shared-target.sh" --print-target-dir)"
  export CARGO_TARGET_DIR
else
  export CARGO_TARGET_DIR
fi
SERVER_BIN="${RTS_SERVER_BIN:-$CARGO_TARGET_DIR/debug/rts-server}"

# --- Options --------------------------------------------------------------------------------
PORT="${PORT:-}"
RUN_RUST_CHECKS=1
RUN_RUST_NEXTEST=1
RUN_SOURCE_SIZE=1
RUN_STATIC_JS=1
RUN_LIVE_NODE=1
RUN_CLIENT=1
RUN_FULL_AI=0
RUN_TRI_STATE_BROWSER=0
RUN_WASM_TRI_STATE="${RTS_RUN_WASM_TRI_STATE:-1}"
NODE_DEPS_READY=0
BROWSER_SCENARIOS="smoke,phase-0.5,phase-2.5,phase-5,phase-3.5,phase-4.5,phase-6"
VERBOSE=0
case "${RTS_RUN_TRI_STATE_BROWSER:-}" in
  1|true|TRUE|yes|YES|on|ON) RUN_TRI_STATE_BROWSER=1 ;;
  0|false|FALSE|no|NO|off|OFF) RUN_TRI_STATE_BROWSER=0 ;;
esac
if [ -n "${GITHUB_ACTIONS:-}" ] || [ -n "${CI:-}" ]; then
  RUN_TRI_STATE_BROWSER=1
fi
for arg in "$@"; do
  case "$arg" in
    --no-rust)   RUN_RUST_CHECKS=0; RUN_RUST_NEXTEST=0 ;;
    --no-client) RUN_CLIENT=0 ;;
    --only-rust) RUN_RUST_CHECKS=1; RUN_RUST_NEXTEST=1; RUN_SOURCE_SIZE=1; RUN_STATIC_JS=0; RUN_LIVE_NODE=0; RUN_CLIENT=0 ;;
    --only-rust-checks) RUN_RUST_CHECKS=1; RUN_RUST_NEXTEST=0; RUN_SOURCE_SIZE=1; RUN_STATIC_JS=0; RUN_LIVE_NODE=0; RUN_CLIENT=0 ;;
    --only-nextest) RUN_RUST_CHECKS=0; RUN_RUST_NEXTEST=1; RUN_SOURCE_SIZE=0; RUN_STATIC_JS=0; RUN_LIVE_NODE=0; RUN_CLIENT=0 ;;
    --only-live-node) RUN_RUST_CHECKS=0; RUN_RUST_NEXTEST=0; RUN_SOURCE_SIZE=1; RUN_STATIC_JS=1; RUN_LIVE_NODE=1; RUN_CLIENT=0 ;;
    --only-browser) RUN_RUST_CHECKS=0; RUN_RUST_NEXTEST=0; RUN_SOURCE_SIZE=0; RUN_STATIC_JS=0; RUN_LIVE_NODE=0; RUN_CLIENT=1 ;;
    --only-browser-scenarios=*) RUN_RUST_CHECKS=0; RUN_RUST_NEXTEST=0; RUN_SOURCE_SIZE=0; RUN_STATIC_JS=0; RUN_LIVE_NODE=0; RUN_CLIENT=1; RUN_TRI_STATE_BROWSER=1; BROWSER_SCENARIOS="${arg#*=}" ;;
    --with-tri-state-browser|--with-tri-state) RUN_TRI_STATE_BROWSER=1 ;;
    --full-ai|--full-selfplay) RUN_FULL_AI=1 ;;
    --port) echo "use --port=N or PORT=N" >&2; exit 2 ;;
    --port=*) PORT="${arg#*=}" ;;
    -v|--verbose) VERBOSE=1 ;;
    -h|--help) sed -n '2,35p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

SELECTED_BROWSER_SCENARIOS=()
IFS=',' read -r -a SELECTED_BROWSER_SCENARIOS <<< "$BROWSER_SCENARIOS"
if [ "${#SELECTED_BROWSER_SCENARIOS[@]}" -eq 0 ] || [ -z "${SELECTED_BROWSER_SCENARIOS[0]:-}" ]; then
  echo "browser scenario shard must include at least one scenario" >&2
  exit 2
fi
for scenario in "${SELECTED_BROWSER_SCENARIOS[@]}"; do
  case "$scenario" in
    smoke|phase-0.5|phase-2.5|phase-5|phase-3.5|phase-4.5|phase-6) ;;
    *) echo "unknown browser scenario: $scenario" >&2; exit 2 ;;
  esac
done

browser_scenario_selected() {
  local wanted="$1"
  local scenario
  for scenario in "${SELECTED_BROWSER_SCENARIOS[@]}"; do
    [ "$scenario" = "$wanted" ] && return 0
  done
  return 1
}

if [ "$RUN_FULL_AI" = "1" ]; then
  echo "running all tests, including full AI coverage, silently; this can take several minutes"
else
  echo "running local test gate silently"
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node not found on PATH — Node/static and browser suites require Node 22.18 or newer." >&2
  exit 2
fi
NODE_VERSION="$(node -p 'process.versions.node' 2>/dev/null || echo 0.0.0)"
IFS=. read -r NODE_MAJOR NODE_MINOR _NODE_PATCH <<< "$NODE_VERSION"

alloc_port() {
  node <<'EOF'
const net = require("node:net");
const s = net.createServer();

s.on("error", (err) => {
  console.error(`could not allocate a free localhost port: ${err.message}`);
  process.exit(1);
});

s.listen(0, "127.0.0.1", () => {
  const address = s.address();
  if (!address || typeof address.port !== "number") {
    console.error("could not allocate a free localhost port: no port assigned");
    process.exitCode = 1;
    s.close();
    return;
  }
  console.log(address.port);
  s.close();
});
EOF
}

if [ -z "$PORT" ]; then
  if ! PORT="$(alloc_port)"; then
    echo "could not allocate a free localhost port; set PORT=<port> and retry" >&2
    exit 2
  fi
fi
if [ -z "$PORT" ]; then
  echo "could not allocate a free localhost port; set PORT=<port> and retry" >&2
  exit 2
fi

HEALTH_URL="http://127.0.0.1:${PORT}/"
export RTS_WS="ws://127.0.0.1:${PORT}/ws"   # consumed by the Node API suites
export RTS_URL="http://127.0.0.1:${PORT}/"  # consumed by client_smoke.mjs
export RTS_MATCH_SEED="${RTS_MATCH_SEED:-1}" # consumed by the server for deterministic tests

# --- Output helpers -------------------------------------------------------------------------
if [ -t 1 ]; then BOLD=$'\033[1m'; RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; RST=$'\033[0m'
else BOLD=""; RED=""; GRN=""; YEL=""; RST=""; fi
hdr()  { [ "$VERBOSE" = "1" ] && printf '\n%s== %s ==%s\n' "$BOLD" "$1" "$RST"; }
info() { [ "$VERBOSE" = "1" ] && printf '%s\n' "$1"; }
warn() { printf '%s! %s%s\n' "$YEL" "$1" "$RST"; }
elapsed_since() { printf '%ss' "$((SECONDS - $1))"; }

if { [ "$RUN_STATIC_JS" = "1" ] || [ "$RUN_LIVE_NODE" = "1" ] || [ "$RUN_CLIENT" = "1" ]; } &&
   { [ "${NODE_MAJOR:-0}" -lt 22 ] || { [ "${NODE_MAJOR:-0}" -eq 22 ] && [ "${NODE_MINOR:-0}" -lt 18 ]; }; }; then
  echo "Node $NODE_VERSION detected; Node/static and browser suites require Node 22.18 or newer." >&2
  exit 2
fi

RTS_NODE_DEPS_CACHE_DIR="${RTS_NODE_DEPS_CACHE_DIR:-/tmp/rts-node-deps}"

rust_tool_version() {
  local tool="$1"; shift
  if command -v "$tool" >/dev/null 2>&1; then
    "$@" 2>&1 | sed -n '1p'
  else
    printf '%s not found on PATH\n' "$tool"
  fi
}

print_rust_test_context() {
  if [ "$RUN_RUST_CHECKS" != "1" ] && [ "$RUN_RUST_NEXTEST" != "1" ]; then
    return 0
  fi
  printf '\nRust test context:\n'
  printf '  CARGO_TARGET_DIR=%s\n' "$CARGO_TARGET_DIR"
  printf '  rustc: %s\n' "$(rust_tool_version rustc rustc --version)"
  printf '  cargo: %s\n' "$(rust_tool_version cargo cargo --version)"
  if command -v cargo-nextest >/dev/null 2>&1; then
    printf '  cargo-nextest: %s\n' "$(cargo nextest --version 2>&1 | sed -n '1p')"
  else
    printf '  cargo-nextest: not found on PATH\n'
  fi
}

print_rust_test_context

TOTAL_START=$SECONDS
FAILED=()        # human-readable names of suites that failed
SKIPPED=()       # suites we deliberately did not run
TIMING_NAMES=()  # suite or phase names with measured durations
TIMING_SECONDS=()
TIMING_STATUS=()
DETAIL_NAMES=()
DETAIL_REAL=()
DETAIL_USER=()
DETAIL_SYS=()

record_timing() {
  TIMING_NAMES+=("$1")
  TIMING_SECONDS+=("$2")
  TIMING_STATUS+=("$3")
}

print_timing_summary() {
  printf '\nCI timing summary:\n'
  local i
  for i in "${!TIMING_NAMES[@]}"; do
    printf '  %-7s %5ss  %s\n' "${TIMING_STATUS[$i]}" "${TIMING_SECONDS[$i]}" "${TIMING_NAMES[$i]}"
  done
  printf '  %-7s %5ss  %s\n' "TOTAL" "$((SECONDS - TOTAL_START))" "tests/run-all.sh"
  if [ "${#DETAIL_NAMES[@]}" -gt 0 ]; then
    printf '\nShell timing details:\n'
    for i in "${!DETAIL_NAMES[@]}"; do
      printf '  real=%7ss user=%7ss sys=%7ss  %s\n' \
        "${DETAIL_REAL[$i]}" "${DETAIL_USER[$i]}" "${DETAIL_SYS[$i]}" "${DETAIL_NAMES[$i]}"
    done
  fi
}

record_detail_timing() {
  local name="$1"
  local timef="$2"
  [ -f "$timef" ] || return 0
  local real user sys
  real="$(awk '$1 == "real" { value = $2 } END { print value }' "$timef" 2>/dev/null)"
  user="$(awk '$1 == "user" { value = $2 } END { print value }' "$timef" 2>/dev/null)"
  sys="$(awk '$1 == "sys" { value = $2 } END { print value }' "$timef" 2>/dev/null)"
  [ -n "$real" ] || return 0
  DETAIL_NAMES+=("$name")
  DETAIL_REAL+=("$real")
  DETAIL_USER+=("${user:-0.000}")
  DETAIL_SYS+=("${sys:-0.000}")
}

# Run a suite, record pass/fail. Args: <name> <command...>
run_suite() {
  local name="$1"; shift
  local logf
  local start=$SECONDS
  logf="$(mktemp -t rts-suite.XXXXXX)"
  [ "$VERBOSE" = "1" ] && hdr "$name"
  if "$@" >"$logf" 2>&1; then
    local elapsed=$((SECONDS - start))
    record_timing "$name" "$elapsed" "PASS"
    rm -f "$logf"
    if [ "$VERBOSE" = "1" ]; then
      info "${GRN}PASS${RST} $name (${elapsed}s)"
    fi
  else
    local rc=$?
    local elapsed=$((SECONDS - start))
    record_timing "$name" "$elapsed" "FAIL"
    warn "FAIL $name (exit $rc, ${elapsed}s)"
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
  local build_start=$SECONDS
  if [ -n "${RTS_SERVER_BIN:-}" ]; then
    if [ ! -x "$SERVER_BIN" ]; then
      record_timing "Server build (debug, prebuilt)" "$((SECONDS - build_start))" "FAIL"
      warn "prebuilt server binary not executable at $SERVER_BIN"
      FAILED+=("server build")
      return 1
    fi
    record_timing "Server build (debug, prebuilt)" "$((SECONDS - build_start))" "SKIP"
    info "using prebuilt server binary at $SERVER_BIN"
  else
    build_log="$(mktemp -t rts-build.XXXXXX)"
    if ! cargo build --manifest-path "$SERVER_DIR/Cargo.toml" -p rts-server --bin rts-server >"$build_log" 2>&1; then
      record_timing "Server build (debug)" "$((SECONDS - build_start))" "FAIL"
      warn "server build failed ($(elapsed_since "$build_start"))"
      cat "$build_log"
      rm -f "$build_log"
      FAILED+=("server build")
      return 1
    fi
    record_timing "Server build (debug)" "$((SECONDS - build_start))" "PASS"
    info "${GRN}PASS${RST} server build ($(elapsed_since "$build_start"))"
    rm -f "$build_log"
  fi
  if [ ! -x "$SERVER_BIN" ]; then
    warn "server binary not found at $SERVER_BIN"
    FAILED+=("server build")
    return 1
  fi

  [ "$VERBOSE" = "1" ] && hdr "Boot server on :$PORT"
  local boot_start=$SECONDS
  SERVER_LOG="$(mktemp -t rts-server-log.XXXXXX)"
  RTS_ADDR="127.0.0.1:${PORT}" RTS_TEST_TICK_MS="${RTS_TEST_TICK_MS:-5}" "$SERVER_BIN" >"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!
  STARTED_SERVER=1

  # Health-check: poll GET / until 200, the server dies, or we time out.
  local deadline=$((SECONDS + 30))
  while ! is_up; do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      warn "server exited during startup; log:"; sed 's/^/  /' "$SERVER_LOG" >&2
      FAILED+=("server boot")
      record_timing "Server boot" "$((SECONDS - boot_start))" "FAIL"
      return 1
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      warn "server did not become healthy within 30s; log:"; sed 's/^/  /' "$SERVER_LOG" >&2
      FAILED+=("server boot")
      record_timing "Server boot" "$((SECONDS - boot_start))" "FAIL"
      return 1
    fi
    sleep 0.3
  done
  if [ "$VERBOSE" = "1" ]; then
    info "server healthy (pid $SERVER_PID) at $HEALTH_URL ($(elapsed_since "$boot_start"))"
  fi
  record_timing "Server boot" "$((SECONDS - boot_start))" "PASS"
}

# Parallel background runner — writes pass/fail result to a temp file.
# Usage: run_suite_bg <name> <command...>
# Caller must call collect_bg_results afterwards.
BG_PIDS=()
BG_NAMES=()
BG_RESULT_FILES=()

run_suite_bg() {
  local name="$1"; shift
  local logf resultf timef
  logf="$(mktemp -t rts-suite.XXXXXX)"
  resultf="$(mktemp -t rts-result.XXXXXX)"
  timef=""
  if [ "${RTS_SUITE_TIMING_DETAILS:-0}" = "1" ]; then
    timef="$(mktemp -t rts-time.XXXXXX)"
  fi
  [ "$VERBOSE" = "1" ] && hdr "$name (bg)"
  (
    start=$SECONDS
    if [ -n "$timef" ]; then
      TIMEFORMAT=$'real\t%3R\nuser\t%3U\nsys\t%3S'
      if { time "$@" >"$logf" 2>&1; } 2>"$timef"; then
        echo ok >"$resultf"
      else
        echo fail >"$resultf"
      fi
    elif "$@" >"$logf" 2>&1; then
      echo ok >"$resultf"
    else
      echo fail >"$resultf"
    fi
    echo "$logf" >>"$resultf"   # second line = log path
    echo "$((SECONDS - start))" >>"$resultf" # third line = elapsed seconds
    echo "$timef" >>"$resultf" # fourth line = optional shell time path
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
    local status logf elapsed timef
    status="$(head -1 "$resultf" 2>/dev/null)"
    logf="$(sed -n '2p' "$resultf" 2>/dev/null)"
    elapsed="$(sed -n '3p' "$resultf" 2>/dev/null)"
    timef="$(sed -n '4p' "$resultf" 2>/dev/null)"
    [ -n "$elapsed" ] || elapsed=0
    [ -n "$timef" ] && record_detail_timing "$name" "$timef"
    if [ "$status" = "ok" ]; then
      record_timing "$name" "$elapsed" "PASS"
      [ -n "$logf" ] && rm -f "$logf"
      [ -n "$timef" ] && rm -f "$timef"
      rm -f "$resultf"
      [ "$VERBOSE" = "1" ] && info "${GRN}PASS${RST} $name (${elapsed}s)"
    else
      record_timing "$name" "$elapsed" "FAIL"
      warn "FAIL $name (${elapsed}s)"
      [ -n "$logf" ] && { cat "$logf"; rm -f "$logf"; }
      [ -n "$timef" ] && rm -f "$timef"
      rm -f "$resultf"
      FAILED+=("$name")
    fi
  done
  BG_PIDS=(); BG_NAMES=(); BG_RESULT_FILES=()
}

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    return 1
  fi
}

hydrate_client_deps() {
  local package_json="$REPO_ROOT/package.json"
  local package_lock="$REPO_ROOT/package-lock.json"
  local local_node_modules="$REPO_ROOT/node_modules"
  local hash cache_dir cache_node_modules ready lock_dir tmp_dir logf
  local start deadline
  local lock_acquired=0

  if [ ! -f "$package_json" ] || [ ! -f "$package_lock" ]; then
    warn "browser dependencies cannot be hydrated: repository package.json or package-lock.json is missing"
    return 1
  fi
  if ! command -v npm >/dev/null 2>&1; then
    warn "client smoke dependencies cannot be hydrated: npm not found on PATH"
    return 1
  fi
  hash="$(hash_file "$package_lock")" || {
    warn "client smoke dependencies cannot be hydrated: no SHA-256 tool found"
    return 1
  }

  cache_dir="$RTS_NODE_DEPS_CACHE_DIR/$hash"
  cache_node_modules="$cache_dir/node_modules"
  ready="$cache_dir/.ready"
  lock_dir="$cache_dir.lock"

  if [ ! -f "$ready" ] || [ ! -d "$cache_node_modules/puppeteer-core" ] || [ ! -d "$cache_node_modules/typescript" ] || [ ! -d "$cache_node_modules/@types/node" ]; then
    mkdir -p "$RTS_NODE_DEPS_CACHE_DIR" 2>/dev/null || {
      warn "client smoke dependencies cannot be hydrated: could not create $RTS_NODE_DEPS_CACHE_DIR"
      return 1
    }

    start=$SECONDS
    deadline=$((SECONDS + 180))
    while ! mkdir "$lock_dir" 2>/dev/null; do
      if [ -f "$ready" ] && [ -d "$cache_node_modules/puppeteer-core" ] && [ -d "$cache_node_modules/typescript" ] && [ -d "$cache_node_modules/@types/node" ]; then
        break
      fi
      if [ "$SECONDS" -ge "$deadline" ]; then
        warn "client smoke dependencies cannot be hydrated: timed out waiting for $lock_dir after $(elapsed_since "$start")"
        return 1
      fi
      info "waiting for client dependency cache lock: $lock_dir"
      sleep 1
    done
    if [ -d "$lock_dir" ] && { [ ! -f "$ready" ] || [ ! -d "$cache_node_modules/puppeteer-core" ] || [ ! -d "$cache_node_modules/typescript" ] || [ ! -d "$cache_node_modules/@types/node" ]; }; then
      lock_acquired=1
    fi

    if [ "$lock_acquired" = "1" ]; then
      tmp_dir="$RTS_NODE_DEPS_CACHE_DIR/.tmp-$hash-$$"
      logf="$(mktemp -t rts-npm-ci.XXXXXX)"
      rm -rf "$tmp_dir" 2>/dev/null
      mkdir -p "$tmp_dir" || {
        warn "client smoke dependencies cannot be hydrated: could not create $tmp_dir"
        rmdir "$lock_dir" 2>/dev/null
        rm -f "$logf" 2>/dev/null
        return 1
      }
      cp "$package_json" "$package_lock" "$tmp_dir/" || {
        warn "client smoke dependencies cannot be hydrated: could not stage package files"
        rm -rf "$tmp_dir" 2>/dev/null
        rmdir "$lock_dir" 2>/dev/null
        rm -f "$logf" 2>/dev/null
        return 1
      }

      info "hydrating client dependency cache: $cache_dir"
      if (cd "$tmp_dir" && npm ci --ignore-scripts --no-audit --fund=false) >"$logf" 2>&1; then
        rm -rf "$cache_dir" 2>/dev/null
        mv "$tmp_dir" "$cache_dir" || {
          warn "client smoke dependencies cannot be hydrated: could not publish $cache_dir"
          cat "$logf"
          rm -rf "$tmp_dir" 2>/dev/null
          rmdir "$lock_dir" 2>/dev/null
          rm -f "$logf" 2>/dev/null
          return 1
        }
        touch "$ready"
        rm -f "$logf"
      else
        warn "client smoke dependencies cannot be hydrated: npm ci failed"
        cat "$logf"
        rm -rf "$tmp_dir" 2>/dev/null
        rmdir "$lock_dir" 2>/dev/null
        rm -f "$logf" 2>/dev/null
        return 1
      fi
      rmdir "$lock_dir" 2>/dev/null
    fi
  fi

  if [ ! -d "$cache_node_modules/puppeteer-core" ] || [ ! -d "$cache_node_modules/typescript" ] || [ ! -d "$cache_node_modules/@types/node" ]; then
    warn "Node dependencies cannot be hydrated: cache is missing puppeteer-core, TypeScript, or Node typings at $cache_node_modules"
    return 1
  fi

  if [ -L "$local_node_modules" ]; then
    local target
    target="$(readlink "$local_node_modules")"
    if [ "$target" != "$cache_node_modules" ]; then
      rm "$local_node_modules" || {
        warn "client smoke dependencies cannot be linked: could not replace $local_node_modules"
        return 1
      }
    fi
  elif [ -e "$local_node_modules" ]; then
    info "replacing local client dependencies with shared cache link: $local_node_modules"
    rm -rf "$local_node_modules" || {
      warn "client smoke dependencies cannot be linked: could not replace $local_node_modules"
      return 1
    }
  fi

  if [ ! -e "$local_node_modules" ]; then
    ln -s "$cache_node_modules" "$local_node_modules" || {
      warn "client smoke dependencies cannot be linked: could not symlink $local_node_modules -> $cache_node_modules"
      return 1
    }
  fi
}

run_nextest_tests() {
  if ! command -v cargo-nextest >/dev/null 2>&1; then
    cat >&2 <<'EOF'
cargo-nextest not found on PATH.
Install it with:
  cargo install cargo-nextest --locked
Then rerun the local Rust gate.
EOF
    return 2
  fi
  local args=(
    --config-file "$REPO_ROOT/.config/nextest.toml"
    --manifest-path "$SERVER_DIR/Cargo.toml"
    --profile default
  )
  if [ -n "${RTS_NEXTEST_PARTITION:-}" ]; then
    args+=(--partition "$RTS_NEXTEST_PARTITION")
  fi
  cargo nextest run "${args[@]}"
}

run_nextest_tests_full_ai() {
  RTS_FULL_AI_TESTS=1 run_nextest_tests
}

find_nextest_junit() {
  local candidate
  for candidate in \
      "$CARGO_TARGET_DIR/nextest/default/junit.xml" \
      "$SERVER_DIR/target/nextest/default/junit.xml" \
      "$REPO_ROOT/target/nextest/default/junit.xml"; do
    if [ -f "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

print_nextest_junit_summary() {
  [ "${RTS_NEXTEST_JUNIT_SUMMARY:-0}" = "1" ] || return 0
  local junit_path
  if junit_path="$(find_nextest_junit)"; then
    node "$SCRIPT_DIR/nextest_junit_summary.mjs" "$junit_path" --limit="${RTS_NEXTEST_JUNIT_LIMIT:-20}" || \
      warn "could not summarize nextest JUnit timing at $junit_path"
  else
    warn "nextest JUnit timing summary requested, but junit.xml was not found"
  fi
}

run_nextest_tests_bg() {
  local name
  if [ "$RUN_FULL_AI" = "1" ]; then
    name="Rust nextest full AI-enabled tests (RTS_FULL_AI_TESTS=1)"
  else
    name="Rust nextest fast scripted tests"
  fi
  if [ -n "${RTS_NEXTEST_PARTITION:-}" ]; then
    name="$name ($RTS_NEXTEST_PARTITION)"
  fi

  if [ "$RUN_FULL_AI" = "1" ]; then
    run_suite_bg "$name" run_nextest_tests_full_ai
  else
    run_suite_bg "$name" run_nextest_tests
  fi
}

run_rust_suites_bg() {
  if [ "$RUN_RUST_CHECKS" = "1" ]; then
    run_suite_bg "Architecture: crate boundaries" \
      node "$REPO_ROOT/scripts/check-crate-boundaries.mjs"
    run_suite_bg "Architecture: sim game internals" \
      cargo run --manifest-path "$SERVER_DIR/Cargo.toml" -p rts-archcheck -- check-sim-architecture
    run_suite_bg "Architecture: client modules" \
      node "$REPO_ROOT/scripts/check-client-architecture.mjs"
    run_suite_bg "Architecture: lobby modules" \
      node "$REPO_ROOT/scripts/check-lobby-architecture.mjs"
    run_suite_bg "Architecture: prediction guardrails" \
      node "$REPO_ROOT/scripts/check-prediction-guardrails.mjs"
    run_suite_bg "Architecture: faction assumptions" \
      node "$REPO_ROOT/scripts/check-faction-assumptions.mjs"
    run_suite_bg "Contract: faction catalog parity" \
      node "$REPO_ROOT/scripts/check-faction-catalog-parity.mjs"
    run_suite_bg "Architecture: structured logging" \
      "$REPO_ROOT/scripts/check-structured-logging.sh"
    run_suite_bg "Architecture: deploy assets" \
      node "$REPO_ROOT/scripts/check-deploy-assets.mjs"
    run_suite_bg "Architecture: test selection policy" \
      node "$SCRIPT_DIR/select-suites.mjs" --verify
    run_suite_bg "Agent workflow: phase runner helper" \
      node "$SCRIPT_DIR/phase_runner_agents.mjs"
    run_suite_bg "Agent workflow: quality pass helper" \
      node "$SCRIPT_DIR/adversarial_quality_pass.mjs"
    run_suite_bg "Agent workflow: configurable PR passes" \
      node "$SCRIPT_DIR/agent_pr_passes.mjs"
    run_suite_bg "Agent workflow: completed plan archival" \
      node "$SCRIPT_DIR/archive_completed_plans.mjs"
    run_suite_bg "Agent workflow: post-merge main refresh" \
      node "$SCRIPT_DIR/wait_pr.mjs"
    run_suite_bg "Rust lint (cargo clippy)" \
      cargo clippy --manifest-path "$SERVER_DIR/Cargo.toml" -- -D warnings
  else
    SKIPPED+=("Architecture policy checks (not selected)")
    SKIPPED+=("Rust lint (not selected)")
  fi

  if [ "$RUN_RUST_NEXTEST" = "1" ]; then
    run_nextest_tests_bg
    if [ "$RUN_FULL_AI" != "1" ]; then
      SKIPPED+=("Rust nextest full AI coverage (--full-ai not set)")
    fi
  else
    SKIPPED+=("Rust nextest fast scripted tests (not selected)")
    SKIPPED+=("Rust nextest full AI coverage (not selected)")
  fi
}

if [ "$RUN_SOURCE_SIZE" = "1" ]; then
  run_suite_bg "Architecture: source file sizes" \
    node "$REPO_ROOT/scripts/check-source-file-sizes.mjs"
else
  SKIPPED+=("Architecture: source file sizes")
fi

if [ "$RUN_STATIC_JS" = "1" ]; then
  deps_start=$SECONDS
  if hydrate_client_deps; then
    NODE_DEPS_READY=1
    record_timing "Node dependency hydration" "$((SECONDS - deps_start))" "PASS"
    run_suite_bg "TypeScript: Interact no-emit" \
      npm --prefix "$REPO_ROOT" run check:interact-types
  else
    record_timing "Node dependency hydration" "$((SECONDS - deps_start))" "FAIL"
    FAILED+=("Node dependency hydration" "TypeScript: Interact no-emit")
  fi
  run_suite_bg "Architecture: Interact application" \
    node "$REPO_ROOT/scripts/check-interact-architecture.mjs"
  run_suite_bg "JS protocol contracts" \
    node "$SCRIPT_DIR/protocol_parity.mjs"
  run_suite_bg "JS client contracts" \
    node "$SCRIPT_DIR/client_contracts.mjs"
  run_suite_bg "JS prediction controller" \
    node "$SCRIPT_DIR/prediction_controller.mjs"
  run_suite_bg "JS tri-state harness self-test" \
    node "$SCRIPT_DIR/tri_state/self_test.mjs"
  run_suite_bg "JS minimap input contracts" \
    node "$SCRIPT_DIR/minimap_input_contracts.mjs"
  run_suite_bg "Interact artifact contracts" \
    node "$SCRIPT_DIR/interact_artifact_contracts.mjs"
  run_suite_bg "Interact bulk contracts" \
    node "$SCRIPT_DIR/interact_bulk_contracts.mjs"
  run_suite_bg "Interact adapter contracts" \
    node "$SCRIPT_DIR/interact_adapter_contracts.mjs"
  run_suite_bg "Interact recording contracts" \
    node "$SCRIPT_DIR/interact_recording_contracts.mjs"
  run_suite_bg "Interact fixed-capture contracts" \
    node "$SCRIPT_DIR/interact_fixed_capture_contracts.mjs"
  run_suite_bg "Interact session coordinator contracts" \
    node "$SCRIPT_DIR/interact_session_coordinator_contracts.mjs"
  run_suite_bg "JS HUD command card" \
    node "$SCRIPT_DIR/hud_command_card.mjs"
  run_suite_bg "Utility: tailnet preview" \
    node "$SCRIPT_DIR/tailnet_preview.mjs"
else
  SKIPPED+=("JS contract suites")
fi

NEEDS_SERVER=0
if [ "$RUN_LIVE_NODE" = "1" ] || [ "$RUN_CLIENT" = "1" ]; then
  NEEDS_SERVER=1
fi

if [ "$NEEDS_SERVER" = "1" ]; then
  # Build the server before Rust suites. In the default local gate, nextest and clippy can then
  # reuse compiled artifacts instead of competing with the live-server build for Cargo's lock.
  if is_up; then
    info "reusing server already listening on :$PORT (will not stop it)"
    SERVER_HEALTHY=1
    run_rust_suites_bg
  else
    if boot_server; then SERVER_HEALTHY=1; else SERVER_HEALTHY=0; fi
    run_rust_suites_bg
  fi
else
  run_rust_suites_bg
fi

# --- 2/3. Node suites + client smoke (all parallelised) ------------------------------------
if [ "${SERVER_HEALTHY:-0}" = "1" ]; then
  if [ "$RUN_LIVE_NODE" = "1" ]; then
  run_suite_bg "API: server_integration" node "$SCRIPT_DIR/server_integration.mjs"
  run_suite_bg "API: regression"         node "$SCRIPT_DIR/regression.mjs"
  run_suite_bg "API: ai_integration"     node "$SCRIPT_DIR/ai_integration.mjs"
  run_suite_bg "API: faction_integration" node "$SCRIPT_DIR/faction_integration.mjs"
  run_suite_bg "API: team_integration"   node "$SCRIPT_DIR/team_integration.mjs"
  run_suite_bg "API: lobby_browser_integration" node "$SCRIPT_DIR/lobby_browser_integration.mjs"
  else
    SKIPPED+=("Live Node API suites")
  fi

  # Browser suites are latency-sensitive and each may drive its own Chrome/WebSocket room. Keep
  # them off the already-parallel background batch so snapshot-lane comparisons are not distorted
  # by local browser/server head-of-line pressure.
  collect_bg_results

  if [ "$RUN_LIVE_NODE" = "1" ]; then
    run_suite "API: lab_mortar_regression" node "$SCRIPT_DIR/lab_mortar_regression.mjs"
  fi

  if [ "$RUN_CLIENT" = "1" ]; then
    # Auto-detect Chrome if not set: macOS app bundle, then common Linux paths.
    if [ -z "${CHROME:-}" ]; then
      for candidate in \
          "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
          "/Applications/Chromium.app/Contents/MacOS/Chromium" \
          "$(which google-chrome-stable 2>/dev/null)" \
          "$(which google-chrome 2>/dev/null)" \
          "$(which chromium-browser 2>/dev/null)" \
          "$(which chromium 2>/dev/null)"; do
        if [ -n "$candidate" ] && [ -x "$candidate" ]; then
          CHROME="$candidate"
          break
        fi
      done
    fi
    if [ -z "${CHROME:-}" ] || [ ! -x "$CHROME" ]; then
      info "skipping browser suites: no Chrome found (set CHROME=/path/to/chrome to override)"
      SKIPPED+=("Client smoke (no Chrome)")
      SKIPPED+=("Interact CLI smoke (no Chrome)")
      SKIPPED+=("Tri-state lag scenarios (no Chrome)")
    else
      deps_start=$SECONDS
      if [ "$NODE_DEPS_READY" = "1" ] || hydrate_client_deps; then
        NODE_DEPS_READY=1
        record_timing "Client dependency hydration" "$((SECONDS - deps_start))" "PASS"
      else
        record_timing "Client dependency hydration" "$((SECONDS - deps_start))" "FAIL"
        FAILED+=("Client smoke dependency hydration")
        FAILED+=("Tri-state scenario dependency hydration")
        collect_bg_results
        hdr "Summary"
        for s in "${SKIPPED[@]:-}"; do [ -n "$s" ] && info "  ${YEL}SKIP${RST} $s"; done
        print_timing_summary
        info "${RED}${#FAILED[@]} suite(s) failed ❌${RST}"
        exit 1
      fi
      if browser_scenario_selected smoke; then
        CHROME="$CHROME" run_suite "Client smoke (headless Chrome)" node "$SCRIPT_DIR/client_smoke.mjs"
        CHROME="$CHROME" RTS_INTERACT_LAB_BASE_URL="$RTS_URL" \
          run_suite "Interact CLI smoke (headless Chrome)" node "$SCRIPT_DIR/interact_cli_smoke.mjs"
      else
        SKIPPED+=("Client smoke (not selected for this browser shard)")
        SKIPPED+=("Interact CLI smoke (not selected for this browser shard)")
      fi
      if [ "$RUN_TRI_STATE_BROWSER" = "1" ]; then
        for scenario in phase-0.5 phase-2.5 phase-5; do
          if browser_scenario_selected "$scenario"; then
            CHROME="$CHROME" run_suite "Tri-state scenarios: ${scenario/phase-/phase }" \
              node "$SCRIPT_DIR/tri_state/run.mjs" --scenario "$scenario"
          fi
        done
        if [ "$RUN_WASM_TRI_STATE" != "1" ]; then
          info "skipping WASM-backed tri-state scenarios: RTS_RUN_WASM_TRI_STATE=$RUN_WASM_TRI_STATE"
          for scenario in phase-3.5 phase-4.5 phase-6; do
            if browser_scenario_selected "$scenario"; then
              SKIPPED+=("Tri-state scenarios: ${scenario/phase-/phase } (RTS_RUN_WASM_TRI_STATE=$RUN_WASM_TRI_STATE)")
            fi
          done
        elif [ -f "$REPO_ROOT/client/vendor/sim-wasm/rts_sim_wasm.js" ] && [ -f "$REPO_ROOT/client/vendor/sim-wasm/rts_sim_wasm_bg.wasm" ]; then
          for scenario in phase-3.5 phase-4.5 phase-6; do
            if browser_scenario_selected "$scenario"; then
              CHROME="$CHROME" run_suite "Tri-state scenarios: ${scenario/phase-/phase }" \
                node "$SCRIPT_DIR/tri_state/run.mjs" --scenario "$scenario"
            fi
          done
        else
          info "skipping WASM-backed tri-state scenarios: generated sim-wasm assets missing"
          for scenario in phase-3.5 phase-4.5 phase-6; do
            if browser_scenario_selected "$scenario"; then
              SKIPPED+=("Tri-state scenarios: ${scenario/phase-/phase } (missing sim-wasm assets)")
            fi
          done
        fi
      else
        info "skipping tri-state browser scenarios locally; use --with-tri-state-browser or RTS_RUN_TRI_STATE_BROWSER=1 to include them"
        SKIPPED+=("Tri-state lag scenarios (local opt-in)")
      fi
    fi
  else
    SKIPPED+=("Client smoke (--no-client)")
    SKIPPED+=("Tri-state lag scenarios (--no-client)")
  fi
elif [ "$NEEDS_SERVER" = "1" ]; then
  warn "server not healthy — skipping all live-server suites"
  SKIPPED+=("API + client suites (server unavailable)")
else
  collect_bg_results
fi

collect_bg_results

# --- Summary --------------------------------------------------------------------------------
hdr "Summary"
for s in "${SKIPPED[@]:-}"; do [ -n "$s" ] && info "  ${YEL}SKIP${RST} $s"; done
print_nextest_junit_summary
print_timing_summary
if [ "${#FAILED[@]}" -eq 0 ]; then
  info "  ${GRN}ALL SUITES PASSED ✅${RST}"
  exit 0
else
  for f in "${FAILED[@]}"; do info "  ${RED}FAIL${RST} $f"; done
  info "${RED}${#FAILED[@]} suite(s) failed ❌${RST}"
  exit 1
fi
