#!/usr/bin/env bash
# Real encoding/tool checks share the selected browser job's FFmpeg installation.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export RTS_RUN_MEDIA_TESTS=1
node tests/interact_recording_contracts.mjs
node tests/interact_fixed_capture_contracts.mjs
node tests/patch_note_outbox.mjs
node --test scripts/minimap-webm/cli.test.mjs
