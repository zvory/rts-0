# Analysis only

This file is interpretation, not raw evidence. The evidence is in the Fly JSONL logs, DB summary,
replay artifacts, parsed network report TSV, and the player-report quotes in this directory.

## Short answer

Both Matt's network path and local client performance explain the complaint. The server does not.

The strongest explanation is that Matt's slow/stalling client made the game feel unplayable, while
network and snapshot jitter made command response worse.

## Why the server is unlikely

The match telemetry does not show the server falling behind:

- Server tick from client reports stayed at `3-6ms`.
- Server lag stayed at `1ms`.
- Slow tick count stayed `0`.
- Head-of-line count stayed `0`.
- Match 54 ran `11914` ticks; at 33 ms/tick that is within 15 ms of the DB duration.
- Match 55 ran `9054` ticks; at 33 ms/tick that is within 14 ms of the DB duration.

If the beta process itself had been overloaded, these fields should have moved first: server tick,
server lag, slow tick count, head-of-line/backlog, or match wall-time drift. They did not.

## Why Matt still felt severe lag

Matt's report had two real backing signals.

Network and snapshot timing:

- Match 54 Matt RTT reached `292ms`, and report-window max RTT reached `368ms`.
- Matt had bad RTT samples in several windows.
- Matt's snapshot jitter reached `104ms`.
- Matt's snapshot gaps reached `516ms`.
- Matt's command acknowledgement latency reached `243ms`.
- In match 55, Matt still had `102ms` RTT, `59ms` jitter, and a `344ms` snapshot gap in the
  notable report.

Local frame pacing:

- Match 54 Matt FPS estimate was mostly `17-23`.
- Matt frame gaps reached `309ms`.
- In match 55, Matt's notable report estimated `23fps`.

Those two classes combine badly. A 150-300 ms network/ack delay already feels mushy for commands.
When the browser is also only painting around 20 fps and occasionally stalls for 150-300 ms, the
same command path feels much worse because local input feedback and visual confirmation are delayed
too.

## Why movement/progress could still look smooth

The reported experience said command response felt bad, but movement and progress bars did not
visibly stutter. That is consistent with the telemetry and the prediction system.

During the bad match 54 windows, Matt's reports mostly still had `predictionMode=tracking`,
`correction_count=0`, and `correction_distance_px=0`. That means prediction was not thrashing
between contradictory authoritative snapshots. It could keep continuous movement/progress visually
smooth enough even while command acknowledgement and authoritative effects felt delayed.

This is the important diagnostic lesson: prediction can mask continuous-motion stutter without
making command response feel instant.

## Label caveat

Do not over-read `primary_issue="prediction_disabled"` in these logs. The underlying
`predictionDisableCount` is cumulative. After one prediction disable, later notable reports can be
classified as prediction-disabled even when the current `predictionMode` is `tracking`.

For this incident, the meaningful fields are:

- `rtt_ms`
- `rtt_max_ms`
- `bad_rtt_samples`
- `snapshot_jitter_ms`
- `snapshot_gap_max_ms`
- `frame_gap_max_ms`
- `fps_estimate`
- `acknowledged_command_latency_ms`
- `server_tick_ms`
- `server_lag_ms`
- `slow_tick_count`
- `head_of_line_count`
- `predictionMode`
- `correction_count`
- `correction_distance_px`

## Takeaway

Classify this as a client/network-path lag incident:

- Server lag: not supported by the evidence.
- Network/snapshot delivery lag: supported for Matt.
- Slow client frame pacing: strongly supported for Matt.
- Prediction health: good enough to mask movement/progress stutter, but not enough to hide delayed
  command response.
