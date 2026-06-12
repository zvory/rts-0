#!/usr/bin/env bash
# Run the whole test suite against a freshly-built, freshly-booted server, then exit
# non-zero if anything failed. This is the canonical "is the build green?" command.
#
# What it runs, in order:
#   1. Architecture policy          (crate boundaries + sim/client architecture + test-selector self-check)
#   2. Rust formatting              (cargo fmt --check)
#   3. Rust fast scripted tests     (cargo test — deterministic, in-process, no server)
#   4. Rust lint                    (cargo clippy)
#   5. Node API suites              (protocol/UI units, server_integration, regression, ai_integration)
#   6. Headless browser suites      (client_smoke + tri-state lag scenarios; needs Chrome)
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
#   tests/run-all.sh -v              # verbose: print headers and passes
#   tests/run-all.sh --no-rust       # skip Rust fmt/test/lint
#   tests/run-all.sh --no-client     # skip the headless-browser smoke test
#   PORT=8090 tests/run-all.sh       # use a different port
#   RTS_MATCH_SEED=123 tests/run-all.sh  # use a different deterministic map seed
#   CARGO_TARGET_DIR=/path/to/target tests/run-all.sh  # override the per-worktree Cargo target dir
#   RTS_NODE_DEPS_CACHE_DIR=/tmp/rts-node-deps tests/run-all.sh
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
SERVER_BIN="$CARGO_TARGET_DIR/debug/rts-server"

# --- Options --------------------------------------------------------------------------------
PORT="${PORT:-}"
RUN_RUST=1
RUN_CLIENT=1
RUN_FULL_AI=0
VERBOSE=0
for arg in "$@"; do
  case "$arg" in
    --no-rust)   RUN_RUST=0 ;;
    --no-client) RUN_CLIENT=0 ;;
    --full-ai|--full-selfplay) RUN_FULL_AI=1 ;;
    --port) echo "use --port=N or PORT=N" >&2; exit 2 ;;
    --port=*) PORT="${arg#*=}" ;;
    -v|--verbose) VERBOSE=1 ;;
    -h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

if [ "$RUN_FULL_AI" = "1" ]; then
  echo "running all tests, including full AI coverage, silently; this can take several minutes"
else
  echo "running local test gate silently"
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node not found on PATH — the API suites need Node >= 22 (built-in WebSocket)." >&2
  exit 2
fi
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"

alloc_port() {
  node -e 'const net = require("node:net"); const s = net.createServer(); s.listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });'
}

if [ -z "$PORT" ]; then
  PORT="$(alloc_port)"
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

if [ "$NODE_MAJOR" -lt 22 ]; then
  warn "Node $NODE_MAJOR detected; the API suites need >= 22 for the global WebSocket. Continuing anyway."
fi

info "Cargo target dir: $CARGO_TARGET_DIR"
RTS_NODE_DEPS_CACHE_DIR="${RTS_NODE_DEPS_CACHE_DIR:-/tmp/rts-node-deps}"

FAILED=()   # human-readable names of suites that failed
SKIPPED=()  # suites we deliberately did not run

# Run a suite, record pass/fail. Args: <name> <command...>
run_suite() {
  local name="$1"; shift
  local logf
  local start=$SECONDS
  logf="$(mktemp -t rts-suite.XXXXXX)"
  [ "$VERBOSE" = "1" ] && hdr "$name"
  if "$@" >"$logf" 2>&1; then
    rm -f "$logf"
    if [ "$VERBOSE" = "1" ]; then
      info "${GRN}PASS${RST} $name ($(elapsed_since "$start"))"
    fi
  else
    local rc=$?
    warn "FAIL $name (exit $rc, $(elapsed_since "$start"))"
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
  build_log="$(mktemp -t rts-build.XXXXXX)"
  if ! cargo build --manifest-path "$SERVER_DIR/Cargo.toml" >"$build_log" 2>&1; then
    warn "server build failed ($(elapsed_since "$build_start"))"
    cat "$build_log"
    rm -f "$build_log"
    FAILED+=("server build")
    return 1
  fi
  info "${GRN}PASS${RST} server build ($(elapsed_since "$build_start"))"
  rm -f "$build_log"
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
    info "server healthy (pid $SERVER_PID) at $HEALTH_URL ($(elapsed_since "$boot_start"))"
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
    start=$SECONDS
    if "$@" >"$logf" 2>&1; then
      echo ok >"$resultf"
    else
      echo fail >"$resultf"
    fi
    echo "$logf" >>"$resultf"   # second line = log path
    echo "$((SECONDS - start))" >>"$resultf" # third line = elapsed seconds
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
    local status logf elapsed
    status="$(head -1 "$resultf" 2>/dev/null)"
    logf="$(sed -n '2p' "$resultf" 2>/dev/null)"
    elapsed="$(sed -n '3p' "$resultf" 2>/dev/null)"
    [ -n "$elapsed" ] || elapsed=0
    if [ "$status" = "ok" ]; then
      [ -n "$logf" ] && rm -f "$logf"
      rm -f "$resultf"
      [ "$VERBOSE" = "1" ] && info "${GRN}PASS${RST} $name (${elapsed}s)"
    else
      warn "FAIL $name (${elapsed}s)"
      [ -n "$logf" ] && { cat "$logf"; rm -f "$logf"; }
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
  local package_json="$SCRIPT_DIR/package.json"
  local package_lock="$SCRIPT_DIR/package-lock.json"
  local local_node_modules="$SCRIPT_DIR/node_modules"
  local hash cache_dir cache_node_modules ready lock_dir tmp_dir logf
  local start deadline
  local lock_acquired=0

  if [ ! -f "$package_json" ] || [ ! -f "$package_lock" ]; then
    warn "client smoke dependencies cannot be hydrated: tests/package.json or tests/package-lock.json is missing"
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

  if [ ! -f "$ready" ] || [ ! -d "$cache_node_modules/puppeteer-core" ]; then
    mkdir -p "$RTS_NODE_DEPS_CACHE_DIR" 2>/dev/null || {
      warn "client smoke dependencies cannot be hydrated: could not create $RTS_NODE_DEPS_CACHE_DIR"
      return 1
    }

    start=$SECONDS
    deadline=$((SECONDS + 180))
    while ! mkdir "$lock_dir" 2>/dev/null; do
      if [ -f "$ready" ] && [ -d "$cache_node_modules/puppeteer-core" ]; then
        break
      fi
      if [ "$SECONDS" -ge "$deadline" ]; then
        warn "client smoke dependencies cannot be hydrated: timed out waiting for $lock_dir after $(elapsed_since "$start")"
        return 1
      fi
      info "waiting for client dependency cache lock: $lock_dir"
      sleep 1
    done
    if [ -d "$lock_dir" ] && { [ ! -f "$ready" ] || [ ! -d "$cache_node_modules/puppeteer-core" ]; }; then
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

  if [ ! -d "$cache_node_modules/puppeteer-core" ]; then
    warn "client smoke dependencies cannot be hydrated: cache missing puppeteer-core at $cache_node_modules"
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

run_rust_suites_bg() {
  if [ "$RUN_RUST" = "1" ]; then
    run_suite_bg "Architecture: crate boundaries" \
      node "$REPO_ROOT/scripts/check-crate-boundaries.mjs"
    run_suite_bg "Architecture: sim game internals" \
      cargo run --manifest-path "$SERVER_DIR/Cargo.toml" -p rts-archcheck -- check-sim-architecture
    run_suite_bg "Architecture: client modules" \
      node "$REPO_ROOT/scripts/check-client-architecture.mjs"
    run_suite_bg "Architecture: prediction guardrails" \
      node "$REPO_ROOT/scripts/check-prediction-guardrails.mjs"
    run_suite_bg "Architecture: test selection policy" \
      node "$SCRIPT_DIR/select-suites.mjs" --verify
    run_suite_bg "Rust format (cargo fmt --check)" \
      cargo fmt --manifest-path "$SERVER_DIR/Cargo.toml" --check
    if [ "$RUN_FULL_AI" = "1" ]; then
      run_suite_bg "Rust full AI-enabled tests (RTS_FULL_AI_TESTS=1 cargo test)" \
        env RTS_FULL_AI_TESTS=1 cargo test --manifest-path "$SERVER_DIR/Cargo.toml"
    else
      run_suite_bg "Rust fast scripted tests (cargo test)" \
        cargo test --manifest-path "$SERVER_DIR/Cargo.toml"
      SKIPPED+=("Rust full AI coverage (--full-ai not set)")
    fi
    run_suite_bg "Rust lint (cargo clippy)" \
      cargo clippy --manifest-path "$SERVER_DIR/Cargo.toml" -- -D warnings
  else
    SKIPPED+=("Architecture policy checks (--no-rust)")
    SKIPPED+=("Rust format (--no-rust)")
    SKIPPED+=("Rust fast scripted tests (--no-rust)")
    SKIPPED+=("Rust lint (--no-rust)")
    SKIPPED+=("Rust full AI coverage (--no-rust)")
  fi
}

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
run_suite_bg "JS HUD command card" \
  node "$SCRIPT_DIR/hud_command_card.mjs"

# --- 1. Build server first (both cargo build and cargo test share the target dir and
#        serialize via cargo's file lock, so we build once, then run both in parallel
#        — cargo test reuses the already-compiled artifacts and mostly just links+runs).
if is_up; then
  info "reusing server already listening on :$PORT (will not stop it)"
  SERVER_HEALTHY=1
  # No build needed; kick off Rust suites immediately if requested.
  run_rust_suites_bg
else
  # Build the server binary first (blocks until done).
  if boot_server; then SERVER_HEALTHY=1; else SERVER_HEALTHY=0; fi
  # Now artifacts are compiled; cargo test and clippy can reuse them with minimal recompilation.
  run_rust_suites_bg
fi

# --- 2/3. Node suites + client smoke (all parallelised) ------------------------------------
if [ "${SERVER_HEALTHY:-0}" = "1" ]; then
  run_suite_bg "API: server_integration" node "$SCRIPT_DIR/server_integration.mjs"
  run_suite_bg "API: regression"         node "$SCRIPT_DIR/regression.mjs"
  run_suite_bg "API: ai_integration"     node "$SCRIPT_DIR/ai_integration.mjs"

  # Browser suites are latency-sensitive and each may drive its own Chrome/WebSocket room. Keep
  # them off the already-parallel background batch so snapshot-lane comparisons are not distorted
  # by local browser/server head-of-line pressure.
  collect_bg_results

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
      SKIPPED+=("Tri-state lag scenarios (no Chrome)")
    elif hydrate_client_deps; then
      CHROME="$CHROME" run_suite "Client smoke (headless Chrome)" node "$SCRIPT_DIR/client_smoke.mjs"
      CHROME="$CHROME" run_suite "Tri-state scenarios: phase 0.5" \
        node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-0.5
      CHROME="$CHROME" run_suite "Tri-state scenarios: phase 2.5" \
        node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-2.5
      CHROME="$CHROME" run_suite "Tri-state scenarios: phase 5" \
        node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-5
      if [ -f "$REPO_ROOT/client/vendor/sim-wasm/rts_sim_wasm.js" ] && [ -f "$REPO_ROOT/client/vendor/sim-wasm/rts_sim_wasm_bg.wasm" ]; then
        CHROME="$CHROME" run_suite "Tri-state scenarios: phase 3.5" \
          node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-3.5
        CHROME="$CHROME" run_suite "Tri-state scenarios: phase 4.5" \
          node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-4.5
        CHROME="$CHROME" run_suite "Tri-state scenarios: phase 6" \
          node "$SCRIPT_DIR/tri_state/run.mjs" --scenario phase-6
      else
        info "skipping WASM-backed tri-state scenarios: generated sim-wasm assets missing"
        SKIPPED+=("Tri-state scenarios: phase 3.5 (missing sim-wasm assets)")
        SKIPPED+=("Tri-state scenarios: phase 4.5 (missing sim-wasm assets)")
        SKIPPED+=("Tri-state scenarios: phase 6 (missing sim-wasm assets)")
      fi
    else
      FAILED+=("Client smoke dependency hydration")
      FAILED+=("Tri-state scenario dependency hydration")
    fi
  else
    SKIPPED+=("Client smoke (--no-client)")
    SKIPPED+=("Tri-state lag scenarios (--no-client)")
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
