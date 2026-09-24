# Minimap WebM experiment

Manual, silent, square 10× replay previews. This experiment changes no deployed code and writes
nothing to the match database. It uses a normal replay spectator connection, captures the replay
once, then renders and compresses locally without repeating simulation for each encoding.

## Run

Requires Node 22.18+, FFmpeg with `libvpx-vp9`, and the optional native Canvas package below.
Run from the repository root. Output files must not already exist.

```sh
scripts/ensure-node-deps.sh --quiet
npm install --prefix /tmp/rts-minimap-canvas --no-audit --no-fund @napi-rs/canvas@1.0.9
mkdir -p /tmp/minimap-export
node scripts/minimap-webm/capture.mjs https://rts-0-zvorygin-beta.fly.dev 387 /tmp/minimap-export/387.jsonl
node scripts/minimap-webm/render.mjs /tmp/minimap-export/387.jsonl /tmp/minimap-export/387.mkv /tmp/rts-minimap-canvas/node_modules/@napi-rs/canvas
node scripts/minimap-webm/encode.mjs /tmp/minimap-export/387.mkv /tmp/minimap-export/387
```

The capture requests 8× playback (the existing server maximum), keeps the connection alive,
selects omniscient vision, and ends at the replay's recorded final tick. Capture time can be much
longer than the finished video. The JSONL stores sample ticks every 10 simulation ticks; the
renderer plays these at 30 fps, giving 10× speed at the game's 30 Hz tick rate. Gaps between
received snapshots hold the previous state rather than changing the output clock. The summary
records the largest snapshot gap. These are sampled network views, not exact offline simulation
frames. A disconnected/failed capture has no completion summary and the renderer rejects it.

The lossless FFV1 `.mkv` is an intermediate, not a sharing format. `encode.mjs` produces four
silent VP9 WebMs plus a JSON size/timing report. All files remain local; use `scripts/tailnet-preview`
for private sharing. Intermediate samples/masters can be deleted after selecting a WebM.

## Visual scope

Terrain colors, tree rendering, and road markings come from the existing client modules. The
entity overlay is deliberately simplified: colored circles for units, squares for buildings,
resource dots, and yellow rings around recent attack targets (received attack events plus sampled HP loss). The timer is simulation time.
This is an omniscient overview, with no camera rectangle, fog, UI panels, chat, or soundtrack.
Attack rings are an approximation, not the player's full under-attack notification policy.
This is a compression/operations experiment, not pixel parity with the production minimap.

## Operational boundary

Capture uses the deployed replay simulation and consumes room CPU while running. Run matches
sequentially and avoid busy periods. It starts and controls the replay room returned by the normal
launch endpoint; it refuses an already-active session or a staging lobby with other viewers. The
20-minute capture deadline fails rather than silently producing a truncated clip. Compatibility
is whatever the selected server supports; the file records the original replay build SHA, but
this path cannot independently verify the original recorded final scores.

Automatic generation, upload, database links, and hover UI are intentionally outside this POC.
A future cheap generation path could record a compact minimap timeline during the original match,
then render that after match end, avoiding a second simulation. That needs its own measurement
of live-tick overhead, a retention policy, and a job/storage design.

## Results — 24 September 2026

Three most recent public beta matches, all recorded by beta build `9fbfb3c60fc2`. Capture ran
sequentially against beta; rendering/encoding ran on the local Mac. Raw measurements are in
[results.json](results.json). KB below means 1,000 bytes; the target is 1,000,000 bytes.

| Match / map | Game time → video | Capture time | 480px, 30fps, CRF32 | 480px, 15fps, CRF44 | 240px, 15fps, CRF40 | 144px, 10fps, CRF44 |
| --- | --- | --- | --- | --- | --- | --- |
| #387 The River | 12:54 → 77.4s | 225.6s | 2,271 KB | **826 KB** | 389 KB | 107 KB |
| #386 Schone Tage | 5:50 → 34.9s | 81.1s | 791 KB | **296 KB** | 136 KB | 41 KB |
| #385 Classic | 8:27 → 50.7s | 94.0s | 1,141 KB | **451 KB** | 219 KB | 65 KB |

Game time is `durationTicks / 30`, not the match-history wall-clock duration. Output duration
rounds to a video frame (at most 0.1s here). All files are silent VP9, 8-bit YUV420p, square.
The encoder uses constant quality (`-b:v 0`), not a guaranteed file-size cap. These three matches
fit under 1 MB at 480px/15fps/CRF44; longer or busier matches may not.

Rendering each 480px lossless master took 2.3–3.7s; encoding the recommended variant took
1.8–3.4s. Initial compilation/dependency setup is excluded. The capture is the dominant elapsed
cost: measured effective playback was 3.4×, 4.3×, and 5.4× despite requesting 8×. All three
completed at the recorded final tick, with maximum source snapshot gaps of two ticks. No new
matches were simulated, persisted, or uploaded: these were spectator replay sessions.

Decoded midpoint frames retain a readable timer, map layout, and player-colored movements at
480px/CRF44, with visible softening of fine terrain and small dots. 240px remains useful as a
thumbnail. 144px is impressively small but sacrifices readability of crowded groups. Attack
highlights are simplified as described above. This sample does not establish production load
capacity or cross-browser playback support.

Suggested next step: use 480px/15fps manually for selected games, keep a 240px option, and inspect
the measured size after each export. Across these examples the selected videos total 1.57 MB,
an average of 0.524 MB each; 1,000 similarly sized previews would occupy about 524 MB before
storage overhead. No storage pricing assumption is needed to evaluate this footprint. Do not
put videos into the existing replay JSON/database blob. If automation becomes desirable, measure
a tiny live minimap-timeline recorder before building replay-resimulation jobs.

Verification: `node --test scripts/minimap-webm/capture.test.mjs` covers game-time resampling,
attack retention across unsampled snapshots, refusal of occupied replay lobbies, and interrupted
capture rejection. All twelve output WebMs decoded fully with FFmpeg without errors. Local
syntax checks and docs-health checks passed. Clips and intermediate replay samples are not
committed; they stay in the local temporary export directory.
