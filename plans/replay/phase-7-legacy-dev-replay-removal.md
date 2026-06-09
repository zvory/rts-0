# Phase 7 - Legacy Dev Replay Removal

## Objective

Remove the old dev/self-play replay artifact format and loader after production replay artifacts
are stable enough to serve every replay entry point. From this phase onward, live matches,
post-match viewing, persisted match-history replays, and dev self-play artifacts should all use the
same versioned replay artifact contract and replay runtime.

## Server Work

- Change scripted self-play and matchup tooling to write `ReplayArtifactV1` instead of the legacy
  dev-only artifact shape.
- Route `/dev/selfplay?replay=...` through the same artifact validation and `ReplaySession`
  construction path used by production replay rooms.
- Keep `/dev/selfplay` as a dev entry point, but make it a convenience wrapper around unified
  replay artifacts rather than a separate replay loader.
- Delete the legacy artifact parser, legacy replay reconstruction code, and any snapshot-full
  playback path that exists only for old saved artifacts.
- Preserve dev-only no-fog live watch behavior only for actively running self-play sessions. Saved
  replay artifacts should use selectable replay vision like every other replay.
- Update panic messages, test failure hints, and documentation that reference the old artifact
  format or imply `/dev/selfplay` can load non-unified artifacts.

## Compatibility Policy

- No migration is required for old local dev/self-play artifacts unless there is a specific
  debugging need before this phase starts.
- If migration is needed, provide a one-off developer conversion tool and remove it before this
  phase is considered complete.
- Production persisted replays remain governed by the explicit replay artifact schema version and
  compatibility checks from earlier phases.

## Verification

- Unit test self-play failure artifact capture writes the unified replay artifact schema.
- Integration or server test `/dev/selfplay?replay=...` loads a unified self-play artifact through
  the shared replay runtime.
- Regression test old legacy artifact payloads are rejected with a clear unsupported-format error
  instead of falling back to legacy playback.
- Search-based cleanup check that no legacy dev replay parser or old artifact-only replay path
  remains outside intentionally named compatibility tests.
- Update testing docs and failure instructions so replay inspection still points developers at
  `/dev/selfplay?replay=<artifact_name>`, but describes the artifact as a normal replay artifact.

## Player-Facing Outcome

No direct gameplay change. Developers and test failures use the same replay artifact system as
players, which reduces replay-only bugs and makes self-play failures inspectable with the normal
viewer controls.
