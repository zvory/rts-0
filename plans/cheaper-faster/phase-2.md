# Phase 2 - Always-On Launcher and Startup UX

## Phase Status

- [ ] Pending.

## Objective

Give users an immediate, truthful startup experience while beta or mainline is stopped. Build one
small always-on launcher that wakes only fixed channel origins, reports progress, and redirects when
the selected server is actually ready. Harden the game client so a short proxy-recognition delay or
first WebSocket failure retries automatically instead of telling users to refresh.

## Preconditions and Approval Gate

- Phase 1 is merged, its head is reachable from `origin/main`, and the beta canary passed.
- The cold-start sample supports a bounded launcher timeout and truthful startup copy.
- Recheck the current `shared-cpu-1x`/256 MB always-on price.
- Obtain explicit user approval before creating the paid launcher app, allocating its Machine, or
  changing any public link or DNS record.

## Work

- Add a launcher service sized for `shared-cpu-1x`/256 MB and kept always on.
- Prefer a minimal nginx-based implementation with static HTML/CSS/JS and fixed server-side wake
  locations. The launcher image should not contain the full game server or require Supabase access.
- Provide explicit beta and mainline choices, current channel descriptions, keyboard/touch
  accessibility, an `aria-live` status, cancel/retry behavior, and direct recovery links.
- Implement fixed same-origin wake endpoints that proxy only to hard-coded beta/mainline readiness
  URLs. Reject caller-controlled hosts, schemes, ports, paths, redirects to untrusted origins, and
  arbitrary proxying.
- Bound connect, response, and total startup timeouts. Distinguish starting, ready, timed out,
  wrong-build, network failure, and maintenance/drain states.
- Redirect only after readiness and a real WebSocket-open gate succeed, or document why the
  launcher cannot perform the latter without consuming a player connection.
- Preserve the selected destination path, query, and fragment through the launcher. Cover replay,
  `rtsLaunch`, room, role, AI, spectator, lab, and replay-artifact URLs without decoding or
  rewriting their gameplay meaning.
- Add client startup state before `Net.connect()` and bounded retry/backoff for initial WebSocket
  connection. Retrying must not create multiple sockets, duplicate global handlers, duplicate joins,
  duplicate launch actions, or leaked timers.
- Replace the current first-connect failure text with startup-aware messaging during the bounded
  wake window. After timeout, retain an actionable retry control and direct status information.
- Do not claim `usually under 5 seconds` unless Phase 1's copy gate passed. Keep a longer technical
  timeout even when typical copy is shorter.
- Add launcher health checks, resource limits, content-security headers, no-store startup responses,
  logs that exclude secrets/query payloads, and a small deploy/rollback path.
- Add a focused cold-start browser harness covering launcher-to-beta and launcher-to-mainline flows.
- Update links and documentation only after the launcher is deployed and verified; direct channel
  URLs remain supported.

## Expected Touch Points

- a new focused launcher directory and Dockerfile/config
- a launcher-specific Fly config
- `deploy.sh` or a narrow launcher deploy helper
- `client/src/app.js`
- `client/src/net.js`
- `client/index.html` and `client/styles.css`
- client contract tests for retry, duplicate suppression, status, teardown, and launch preservation
- browser smoke or a focused cold-start browser harness
- `scripts/check-deploy-assets.mjs`
- `docs/context/deployment.md`
- `docs/design/client-ui.md` if App/Net export or lifecycle contracts change
- `plans/cheaper-faster/phase-2.md` status update in the implementation commit

## Security and Correctness Requirements

- The launcher is not an open proxy and never fetches a user-supplied origin.
- Do not log player names, room names, replay ids, arbitrary query strings, secrets, or full target
  URLs.
- The launcher must not join rooms, hold a game WebSocket open after redirect, or keep a game
  Machine running after the user leaves.
- Client retry must allocate one active socket at a time and cancel cleanly on navigation/destroy.
- A launcher failure must not make direct beta/mainline URLs unusable.
- The launcher and game apps remain independently deployable and rollbackable.

## Explicit Exclusions

- No reverse proxy for live game WebSockets or snapshots.
- No combined beta/mainline game process.
- No authentication or account system.
- No CDN migration of the complete game client unless the simple launcher proves insufficient.
- No mainline performance/autostop change in this phase.

## Implementation Checklist

- [ ] Freeze allowlisted channel origins and redirect rules.
- [ ] Build the minimal accessible launcher UI and fixed wake endpoints.
- [ ] Add launcher security headers, timeouts, health checks, and logs.
- [ ] Add initial WebSocket retry and startup-aware client state.
- [ ] Add duplicate-socket/join/timer regression coverage.
- [ ] Add deep-link preservation coverage.
- [ ] Run launcher and client focused checks locally.
- [ ] Obtain explicit approval and deploy the launcher at the smallest current Fly size.
- [ ] Run real stopped-beta and warm-beta browser flows on desktop and phone.
- [ ] Confirm launcher cost/spec and direct URL recovery.
- [ ] Mark this phase done in the implementation commit.

## Verification

Run the relevant selector-chosen suites plus:

```bash
node scripts/check-client-architecture.mjs
node tests/select-suites.mjs --verify
node scripts/check-deploy-assets.mjs
node scripts/check-docs-health.mjs
git diff --check
```

Run focused launcher tests against stub upstreams for starting, ready, timeout, wrong build,
upstream redirect, invalid method, and untrusted-target attempts. Run a real beta cold-start browser
sample only after the launcher PR is merged and its paid deployment is explicitly approved.

## Manual Test Focus

From a phone and desktop with beta stopped, choose beta in the launcher and verify the status is
visible immediately, updates without flicker, and redirects once to the requested deep link. Repeat
for warm beta, timeout/retry, browser back, and direct channel URL. Verify keyboard, touch, reduced
motion, screen-reader announcements, and that leaving the launcher does not keep beta running.

## Handoff Expectations

Report launcher app size and cost, deployed build, allowlist, cold/warm timings, WebSocket retry
behavior, deep-link matrix, accessibility result, and all failure-state copy. State whether the
measured data supports `usually under 5 seconds`, identify the exact mainline rollout steps for
Phase 3, and include launcher and direct-URL rollback procedures.
