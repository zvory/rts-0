# Client stress-test reports

This document owns the cross-file contract for the shareable client-only Hellhole benchmark:
`/stress-test`, `StressTestRunner`, the stress-test HTTP API, and `client_stress_tests` persistence.

## Route and workload

`/stress-test` automatically launches the checked-in `supply-300-hellhole.rtsstream` through
`SnapshotStreamNet`. It uses the normal snapshot decode, `GameState`, `Match`, Pixi module worker, fog, HUD,
minimap, and animation-frame paths, but never opens a WebSocket or runs a server simulation.
The canonical recording starts with 420 projected entities: the 300-supply 2v2 armies and structures
plus Player 1's 120 map-wide, deterministically scattered Tank Traps.
`?label=<text>` adds a bounded human label to every artifact. `?seconds=<2..25>` exists for local
iteration; the shareable default is five seconds after a three-second warmup. The cap keeps the
measurement inside the finite 30-second recording.

The route response alone sends `Document-Policy: js-profiling`. The pinned Pixi ESM module is loaded
inside the render worker. Chromium browsers with the JS Self-Profiling API collect the main-page
10 ms sampled trace and SVG flame graph; that browser API does not make the page trace a worker CPU
profile. Browsers without it still complete the run with the same bounded main-thread phase/frame
summary, render-worker queue/timing diagnostics, and long-frame observations. The canonical CLI
harness profiles the page and render worker independently.

## Measurement and validity

The runner mounts only a compact progress/result status surface; there is no preflight form. It
starts after `App.onStart` has constructed the ordinary `Match`, warms the renderer and assets for
three seconds, then resets `Match.frameProfiler` immediately before measuring. Stream download,
parsing, initial Pixi allocation, and shader warmup are therefore outside the result.

The report includes main-thread frame-work, renderer submission, fog, scheduling, and
diagnostic-counter summaries from `FrameProfiler`; render-worker submitted/completed/superseded/
failed counts and queue/display/main-submit/worker-update/worker-present timings; actual average
completed-presentation throughput; the static stream
identity; Long Tasks and Long Animation Frames
when supported; and a JS trace/flame graph when supported. The result UI reports the p95 frame-work
tier and the approximate work reduction or headroom against 16.67 ms. This is a relative
frame-work indicator, not a claim that the display actually presented at 120 or 240 Hz.
Worker display age covers the complete interval from host acceptance through acknowledgment,
including bounded host-pending time, message construction/cloning, dispatch, worker update, and
presentation. Queue age uses the same acceptance boundary through worker task start.

Warmup and measurement require an uninterrupted visible, focused tab. If either condition changes,
the current attempt and its browser profile are discarded without uploading; the runner waits for
the tab to return, rebuilds the match from the recording's first snapshot, repeats the full warmup,
and starts a fresh five-second measurement. A completed
foreground window with at least one rendered frame is accepted, so a truly slow machine remains
measurable; fewer frames trigger another local restart instead of a misleading zero-FPS artifact.

## Identification and privacy

The page accepts an explicit `label`, creates a random stable device id in localStorage, and hashes
a coarse environment fingerprint. It records browser/UA client hints when available, OS/platform,
CPU architecture/bitness, logical cores, approximate memory, WebGL vendor/renderer, screen and
viewport dimensions, DPR, estimated refresh rate, locale/timezone, and coarse Network Information
API values. Browsers do not expose an OS username or personal name. The route does not request
geolocation permission and the server does not store a raw client IP in the artifact.

## HTTP and persistence

`POST /api/stress-tests` accepts schema v1 and issues both an unguessable run id and a filename-safe
artifact label. `GET /api/stress-tests/{run_id}` downloads the complete labeled JSON artifact;
`GET /api/stress-tests/{run_id}/flamegraph.svg` downloads the SVG when a JS profile exists. The
server logs one structured headline row for every accepted report and retains the latest 64
artifacts in process memory, so local end-to-end testing works without a database.

Postgres persistence is independent of match-history writes. It occurs only when both
`DATABASE_URL` is available and `RTS_RECORD_STRESS_TESTS` is truthy. Local `cargo run` should leave
the gate off; beta/mainline should enable it when shareable artifacts must survive restarts. The
`client_stress_tests` table stores indexed throughput/p95 headline columns plus the complete JSON artifact. Run-id
lookups check the bounded memory cache first and Postgres second whenever a database is configured,
even when new writes are gated off. There is intentionally no public list endpoint.

## Untrusted-input limits

The POST body is capped at 2 MiB before JSON extraction. The server validates the workload and
schema, label/device/fingerprint lengths and character sets, 1.5–30 second duration, status,
invalid-reason count, profile kind, trace-table sizes (4,000 samples, 12,000 stacks/frames, 1,000
resources), and a 750 KiB SVG cap. The server, not the browser, generates run ids, durable artifact
labels, build ids, and receive timestamps. The SVG validator rejects scriptable elements,
event-handler attributes, and links before the same-origin download endpoint can serve it. The SVG
response also applies a restrictive CSP sandbox and MIME-sniffing protection.
Retrieval accepts only the run-id shape generated by the server.
