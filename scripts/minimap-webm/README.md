# Manual minimap WebM recording

Manual, silent, square 15× replay previews with combined authoritative fog. Nothing runs
automatically: select one match whenever you want a recording. The tool uses an ordinary replay
spectator connection, then renders and compresses locally. It writes nothing to the match database.

## Run

Requires Node 22.18+, FFmpeg with `libvpx-vp9`, and ffprobe. From the repository root:

```sh
node scripts/record-minimap.mjs 387 /tmp/match-387.webm
```

The default server is beta. Use `--server http://localhost:8080` or another deployment origin
for matches stored elsewhere. The output directory must exist; existing output files are never
overwritten. First use installs the optional `@napi-rs/canvas@1.0.9` package into
`~/.cache/rts-minimap-webm`, separately from the game's dependencies, and prepares repository Node
dependencies if needed. To use an existing Canvas installation, pass `--canvas-package /absolute/path/to/node_modules/@napi-rs/canvas`.

The command captures, renders, encodes, verifies that the entire video decodes, and prints its
path and size. It produces 480×480, 15 fps, VP9 CRF44 with a clock-only overlay at 15× speed.
A constant-quality encode does not guarantee a file-size cap. Temporary captures and lossless
masters are removed after success; `--keep-work` retains them and prints their directory.
Failures retain working files for diagnosis. Ctrl-C stops the active child and leaves working
files in the printed directory; an interrupted capture cannot be rendered.

To re-render a completed capture without running the replay again:

```sh
node scripts/record-minimap.mjs --from-samples /tmp/rts-minimap-EXAMPLE/capture.jsonl /tmp/another.webm
```

### Unit PNG presentation

Add `--unit-pngs` to either capture or re-render a video with the approved unit-portrait style:

```sh
node scripts/record-minimap.mjs 387 /tmp/match-387-units.webm --unit-pngs
node scripts/record-minimap.mjs --from-samples /tmp/capture.jsonl /tmp/units.webm --unit-pngs
```

At 480px, riflemen use a roughly 12px portrait and tanks 35px, with a 1px white
silhouette outline. Whole portraits follow recorded body facing; machine-gunner art receives a
90° counterclockwise correction. Turrets do not aim independently. Buildings retain the regular
colored, white-outlined boxes, and terrain, fog, resources, attack notices, and the clock retain
the normal exporter behavior. HUD art without a PNG route uses its existing SVG portrait.
The normal marker style remains the default.

This option installs optional `sharp@0.34.5` alongside Canvas in `~/.cache/rts-minimap-webm` to
rasterize the HUD portraits' SVG tint filters correctly. Sprites are loaded as unit kinds/team
colors appear and cached for the export; no capture-specific paths or samples are required.
Larger portraits can overlap in groups and increase the encoded file size. The reviewed
51.7-second The River example was about 2.29 MB at the usual VP9 settings.

For compression experiments on a retained master, call the underlying encoder directly:

```sh
node scripts/minimap-webm/encode.mjs /tmp/rts-minimap-EXAMPLE/master.mkv /tmp/comparison all
```

For a private viewing link, run `scripts/tailnet-preview /tmp/match-387.webm` (24-hour default).
The recorder does not upload, schedule jobs, change match history, or create hover previews.

The capture requests 8× playback (the existing server maximum), keeps the connection alive,
selects combined player vision, and ends at the replay's recorded final tick. Capture time can be much
longer than the finished video. The JSONL stores sample ticks every 10 simulation ticks; the
renderer plays these at 45 fps before the compact 15 fps encode, giving 15× speed at the game's 30 Hz tick rate. Gaps between
received snapshots hold the previous state rather than changing the output clock. The summary
records the largest snapshot gap. These are sampled network views, not exact offline simulation
frames. A disconnected/failed capture has no completion summary and the renderer rejects it.

The lossless FFV1 `.mkv` is an intermediate, not a sharing format. `encode.mjs` produces four
silent VP9 WebMs by default, or the compact 480px and 240px variants with the `compact` argument, or a single named variant such as `480-q44`, plus a JSON size/timing report. All files remain local; use `scripts/tailnet-preview`
for private sharing. Intermediate samples/masters can be deleted after selecting a WebM.

## Visual scope

The exporter calls the production `Minimap.render()` directly, using the regular `GameState`,
`Fog`, and `MatchNoticePresenter` classes. `regular-minimap.mjs` supplies a native Canvas host,
loads the existing artillery icon, and advances a deterministic game-time clock. It contains no
second implementation of terrain, fog, entity markers, or attack rings. The only export-specific
drawing is the MM:SS clock. Future minimap drawing changes therefore flow into exports too.

Capture schema 2 preserves full entity/snapshot fields, resource deltas, and real timed events;
old simplified captures must be recaptured. Combined server visibility stays clear, explored
terrain is dimmed, and unexplored terrain is darkened. The 480×480 backing canvas preserves the
regular 220px minimap's UI proportions. There is no camera rectangle because the exporter has no
camera, and no speed label, chat, UI panels, or soundtrack. Native Canvas rasterization can differ
slightly from browser Canvas; this is shared rendering code, not a claim of browser pixel parity.

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

## Initial 10×, omniscient results — 24 September 2026

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
highlights in this historical iteration were simplified. This sample does not establish production load
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

## Local 15× combined-fog iteration

This unmerged experiment uses authoritative union-of-player visibility/exploration grids and a
clock-only overlay. Compact encodes remain VP9 at 15 fps. All six WebMs decoded fully; fog grids
were checked for size, continuous sample timing, and visible-tile containment within explored
terrain. The six sampling/capture tests passed, and decoded midpoint frames were inspected.

| Match | Duration | 480px CRF44 | 240px CRF40 |
| --- | --- | --- | --- |
| #387 The River | 51.6s | 950,788 bytes | 503,687 bytes |
| #386 Schone Tage | 23.3s | 310,711 bytes | 150,029 bytes |
| #385 Classic | 33.8s | 465,965 bytes | 250,283 bytes |

Capture took 236.3s, 95.7s, and 92.6s respectively. Fog rendering took 20.1s, 22.8s,
and 15.2s; each selected 480px encode took 1.6–3.0s. Moving fog edges increase bitrate, so these
shorter videos are slightly larger than the initial 10× omniscient examples at the same quality.
All three 480px outputs still fit below 1,000,000 bytes. No commits, pushes, or merges were made
for this iteration; the existing PR's auto-merge remains disabled.

## Local 15× production-renderer iteration

Uses the existing Minimap renderer and client fog/state/notice classes via a native Canvas host.
No production client code changed. Combined authoritative fog, game-time clock only, silent VP9,
480×480, 15 fps, CRF44. Full measurements: [results-regular15.json](results-regular15.json).

| Match | Video duration | Bytes | Capture | Render | Encode |
| --- | --- | --- | --- | --- | --- |
| #387 The River | 51.6s | 1,155,456 | 228.1s | 14.9s | 3.2s |
| #386 Schone Tage | 23.3s | 382,506 | 87.1s | 7.7s | 1.4s |
| #385 Classic | 33.8s | 544,578 | 90.4s | 9.6s | 2.0s |

All three files decoded fully without errors; decoded midpoint images were inspected. The game
renderer gives larger, white-outlined markers and real attack notifications. The clock and player
colors remain readable, with compression softening fine terrain. The longest clip exceeds 1 MB.
Seven sampling/capture tests and the production minimap input contracts passed. No commit, push,
or merge was performed; PR #1605 remains open with auto-merge disabled.
