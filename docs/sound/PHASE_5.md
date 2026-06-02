# Phase 5 — Replay determinism & polish

Last mile. Only worth doing if replays are in scope.

- Replace any remaining `Math.random()` in audio code with the seeded stream.
- Replay player must feed the audio module the same event stream + same seed; verify audio output
  byte-identical between two replays (assert via the test stub's call log).
- Add a `--mute` query-string flag to the self-play replay route (`/dev/selfplay?replay=…&mute`)
  for headless debugging.
- Settings polish: per-category sliders, mute toggle (M key), audio device selector if Web Audio
  exposes one on the target browsers (Chrome does via `setSinkId`).

Deliverable: replays sound the same on every machine; debugging flags exist.
