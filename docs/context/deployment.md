# Capsule: deployment & hardening

Use when changing server bind/config, the build pipeline, or the input-hardening surface.

## Read first
- [docs/design/hardening.md](../design/hardening.md) — full hardening section

## Run / build
```bash
cd server && cargo run            # serves client + /ws on RTS_ADDR; open the printed URL
cd server && cargo run --release  # fast build
cd server && cargo build && cargo clippy && cargo fmt
node scripts/check-wiki.mjs       # wiki routes, generated stats, and catalog parity
node scripts/check-crate-boundaries.mjs
```

No JS build step (plain ES modules + PixiJS from CDN). The client is served from `../client`
relative to the server crate, so `cargo run` from `server/` is the whole dev loop.

## Invariants
- **Clients are untrusted.** Validate and bound every wire input:
  - ordinary command unit lists deduped + capped (`MAX_UNITS_PER_COMMAND`); lab command-limit
    bypass lists use the larger bounded `LAB_MAX_UNITS_PER_COMMAND`
  - WebSocket frames size-limited
  - placement coords range/overflow-checked
- **No panics on tick or network paths.** Stale ids = no-op. Use `checked_*` arithmetic on
  anything derived from client input. Debug builds have overflow checks **on** (a bad `Build`
  coord can panic in `cargo run` but silently wrap in `--release`). Keep placement math
  `checked_*`.
- Keep the room task alive: handle errors, don't propagate panics out of message handlers.
- `rts-server` is the only crate that may own Axum/Tokio WebSocket/static-file serving.
- `/wiki` is server-rendered and read-only. It may serve only allowlisted Markdown from
  `docs/context` and `docs/design`; `/wiki/stats` must be generated from `rts-rules` definitions
  and faction catalogs, not scraped from client config or rendered docs.
- Lab scenario PR submission is disabled unless server env explicitly enables it and supplies
  server-side GitHub credentials. The browser probes only `/api/lab-scenarios/submission`; actual
  submissions are lab-room requests that export authoritative game state, write only the scenario
  JSON plus manifest allowlist, recheck duplicate/path/payload/entity caps, and run GitHub work
  outside the room tick path. Live submissions require `RTS_SCENARIO_PR_ENABLED=1`,
  `RTS_SCENARIO_PR_GITHUB_TOKEN`, `RTS_SCENARIO_PR_REPO`, optional base branch/prefix env vars,
  and `git` plus GitHub CLI (`gh`) on the server host.

## Cross-capsule triggers
- Touching the wire surface → [protocol.md](protocol.md).
- Touching sim correctness → [server-sim.md](server-sim.md).
