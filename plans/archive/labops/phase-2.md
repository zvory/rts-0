# Phase 2 - Discoverability, Large-Scene Bounds, And Daemon Freshness

Status: done.

## Objective

Make the Lab CLI self-describing, comfortable with large bounded scenes, and explicit when its
background daemon was started from a different checkout commit.

## Work

- Add daemon-free `help <command>` and `<command> --help` forms. Each descriptor has a summary,
  exact accepted shape/variants, defaults, bounds, and one JSON example; descriptor coverage must
  equal the public command catalog.
- Raise operational aliases, inspection references/results, camera focus references, screenshot
  subjects, and related bridge/driver entity-id bounds to 400 where Phase 1 has not already done so.
- Keep screenshot/recording/fixed-capture manifests bounded using subject/alias counts,
  `truncated` metadata, and a smaller documented detailed-summary cap.
- Confirm the alias sidecar byte cap safely admits 400 maximum-length aliases or adjust the bounded
  sidecar cap with evidence.
- Capture `git rev-parse HEAD` when the daemon starts and publish it through authenticated daemon
  state/probe data. The CLI compares that value with the current checkout before dispatch.
- Keep `status` and `shutdown` available across mismatch. Auto-restart only an idle daemon with no
  active session/request; otherwise return `daemonCheckoutMismatch` containing both commits and the
  safe recovery command.
- Treat a pre-feature daemon missing checkout metadata as a mismatch that can still be inspected
  and shut down, not as an unauthenticated or unrelated listener.
- Preserve IPC v1 while adding the optional checkout metadata; do not fold the checkout SHA into
  daemon authentication/identity validation, which would make mismatch `status` and `shutdown`
  impossible.
- Do not attempt dirty-worktree source hashing or live process replacement in this phase.

## Expected Touch Points

- a focused command-help metadata module plus `scripts/lab-interact/cli.mjs`
- command service, driver bounds, recording/fixed manifest summary helpers, and fake driver
- `client/src/lab_interact_bridge.js`
- daemon runtime/state/probe lifecycle and CLI contracts
- Lab Interact CLI/driver/recording/fixed-capture contracts and smoke coverage
- CLI documentation and the Lab Interact skill

## Verification

- Help works outside a Git checkout, starts no daemon, supports both syntaxes, and covers every
  command; malformed/unknown commands remain concise JSON failures where appropriate.
- 400-reference aliases/inspect/focus/screenshot operations succeed and 401 are rejected before the
  bridge; responses and manifests remain within documented bounded summaries.
- Daemon tests cover matching checkout, old-daemon missing metadata, idle mismatch refresh, active
  mismatch protection, `status`, `shutdown`, and stale session ids after deliberate refresh.
- A live canary bulk-spawns a large scene through Phase 1, focuses the full authored subject set,
  captures one clean PNG, and confirms readiness checks cover all requested subjects without
  returning 400 detailed summaries.
- Run focused client architecture, Lab Interact contracts/smokes, docs health, suite selection, and
  the owned-PR workflow.

## Manual Testing Focus

Use command help from a fresh shell to discover `time`, bulk mutation, focus, and screenshot shapes,
then capture a scene with more than 20 subjects. Start a daemon, advance the checkout commit, and
confirm the active-scene warning preserves the scene while idle recovery refreshes safely.

## Handoff

Report the final help syntax, operational and summary limits, mismatch response/recovery behavior,
exact tests, and the large-scene capture path. Tell Phase 3 the final daemon timeout plumbing and
manifest alias/subject caps it must preserve.
