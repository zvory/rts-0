#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$repo_root"

violations="$(
  rg -n \
    -g '*.rs' \
    -g '!server/src/structured_log.rs' \
    -g '!server/crates/sim/src/perf.rs' \
    '(^|[^[:alnum:]_:])(trace|debug|info|warn|error)!\(|tracing::(trace|debug|info|warn|error)!|use tracing::.*\b(trace|debug|info|warn|error)\b' \
    server || true
)"

if [[ -n "$violations" ]]; then
  cat >&2 <<'EOF'
Direct server logging is not allowed.

Use the structured logging helper instead:
  crate::log_info!(...)
  crate::log_warn!(...)
  crate::log_error!(...)
  crate::log_debug!(...)

In server/src/main.rs, use the library-qualified forms:
  rts_server::log_info!(...)
  rts_server::log_warn!(...)
  rts_server::log_error!(...)
  rts_server::log_debug!(...)

Add new high-signal structured event functions to server/src/structured_log.rs when a log needs
stable fields, issue classification, or cross-event correlation.

The only direct tracing exception is server/crates/sim/src/perf.rs, which is the centralized
simulation performance logging surface and cannot depend on the server crate.

Violations:
EOF
  printf '%s\n' "$violations" >&2
  exit 1
fi

echo "structured logging check passed"
