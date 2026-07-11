# Cheaper, Faster Hosting Plan

## Purpose

Move beta and mainline from always-on shared CPU Machines to Fly `performance-1x` Machines that
stop when idle. Put a tiny always-on launcher in front so friends see a useful starting screen
instead of a failed connection while a game server wakes up.

This is a pre-alpha game for a small group of friends. The standard is that the ordinary workflow
feels good, costs less at the actual play schedule, and is easy to undo—not production-grade uptime
or statistically proven reliability.

## Intended Result

- Beta and mainline remain separate Fly apps and releases named `bewegungskrieg-beta` and
  `bewegungskrieg-mainline`.
- Each game app uses one `performance-1x`/2 GB Machine with autostart and clean autostop.
- One cheap always-on launcher owns `https://bewegungskrieg.net` and
  `https://www.bewegungskrieg.net`, offers fixed beta and mainline choices, shows
  `Starting server...`, and redirects when the chosen game server responds.
- Mainline is served directly at `https://bewegungskrieg-mainline.fly.dev` and beta at
  `https://bewegungskrieg-beta.fly.dev`.
- The client retries its initial WebSocket connection for a short bounded period so a normal cold
  start does not require a manual refresh.
- Existing `bewegungskrieg.net` deep links continue to work: the launcher treats an unqualified
  game deep link as mainline, preserves its path and query, wakes mainline, and redirects there.
- Existing Squarespace DNS and Fly certificates remain untouched. The current canonical-domain
  Fly app is repurposed as the launcher after both newly named game apps are verified.

## Constraints

- Do not combine beta and mainline or change gameplay, simulation, fog, protocol, or balance.
- Use `stop`, not `suspend`.
- Do not intentionally stop or resize a Machine with an active room.
- The launcher may wake only hard-coded beta and mainline origins; it must not accept an arbitrary
  upstream URL.
- The launcher is a separate Fly app, not a process behind the mainline Machine. It must remain
  available while both game Machines are stopped and must redirect rather than proxy live game or
  WebSocket traffic.
- Do not require Squarespace DNS changes. Use the two new apps' raw Fly hostnames as fixed game
  destinations and keep the existing canonical-domain app name for the launcher.
- Headless AI rooms without a connected browser may not prevent Fly autostop. That is acceptable
  for this project and should be documented plainly.
- Keep the existing shutdown/drain behavior. Preserve the old beta app and the temporary launcher
  app in a stopped state until acceptance completes so rollback does not depend on DNS changes.
- Recheck Fly pricing before remote changes. Creating the launcher app, resizing a Machine,
  moving certificates or DNS, or changing a paid remote configuration requires explicit user
  approval immediately beforehand.
- Each phase gets its own branch and owned PR. Wait for it to merge before starting the next phase.
- After each phase, report what changed, what the agent actually tried, whether it worked, and the
  rollback command.

## What Counts as Working

- From a stopped beta Machine, the agent can use the launcher, see a starting state, reach the
  requested game page, and connect without manually refreshing.
- Opening `https://bewegungskrieg.net` always reaches the launcher even when both game Machines are
  stopped; an existing canonical mainline deep link survives the wake-and-redirect flow.
- The agent can enter a lobby and play or spectate a short match without the Machine stopping.
- After all browser connections close, the Machine eventually stops.
- Repeating the cold-start workflow once does not expose an obvious intermittent failure.
- Beta and mainline still deploy and report their own builds independently.
- The projected launcher plus game compute is cheaper than the current always-on setup at the
  expected play schedule.

If beta fails those checks or the workflow is annoying, fix the obvious issue once or roll beta
back. Do not build a measurement program around it.

## Phase Summaries

### [Phase 1 - Build and Try It on Beta](phase-1.md)

Add the small launcher, initial WebSocket retry, and separate beta/mainline Fly configuration. Run
existing selected CI plus a small security test for the launcher's fixed destinations. The
implementation merged, and the beta performance/autostop config was proven deployable before the
user chose the no-DNS app-role swap completed in Phase 2.

### [Phase 2 - Roll It Out to Mainline](phase-2.md)

Create the newly named beta and mainline game apps, deploy both with performance-autostop, and then
repurpose the existing canonical-domain app as the always-on launcher without changing Squarespace
DNS. The agent cold-starts and exercises both destinations through the launcher, confirms idle
stop and release isolation, records rollback commands, and finishes the plan.

## Testing Philosophy

Let the repository's existing selected CI catch ordinary regressions. Add one focused launcher test
that proves callers cannot choose an arbitrary upstream; validate the Fly configs before deploying.

The agent-run deployed workflow is the acceptance test. Do not build new automated suites for
startup timing, retry behavior, URL matrices, accessibility, billing, provider comparisons, or
mocked failure states unless the manual beta attempt exposes a real bug that needs a regression
test.

## Non-Goals

- Production uptime or an SLA.
- A promise that startup takes a particular number of seconds.
- Statistical cold-start or reliability analysis.
- A CDN, general reverse proxy, multi-Machine routing, or durable headless rooms.
- Provider migration or a long-term hosting study.

## Executor Commands

After review and approval, execute one merged phase at a time:

```bash
scripts/phase-runner.sh --plan cheaper-faster phase-1 --pr --wait
scripts/phase-runner.sh --plan cheaper-faster phase-2 --pr --wait
```
