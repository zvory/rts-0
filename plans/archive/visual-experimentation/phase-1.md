# Phase 1 - Profile Launch And Registry

## Phase Status

Status: done.

## Objective

Introduce local visual profile launch plumbing without drawing any candidates yet. A lab URL can
carry one sanitized `visualProfile` id, `App` can resolve that id through a checked-in registry, and
`Match` can receive the resolved profile as ordinary injected local state.

## Scope

- Extend the lab launch parser so `/lab?...&visualProfile=<id>` is recognized only on lab routes.
  Keep the accepted token small and explicit, for example alphanumeric plus `_` and `-` with a
  bounded length.
- Keep `visualProfile` out of the server lab room string. The server should continue to see an
  ordinary `__lab__:<room>:map=<map>...` room with no visual experiment data.
- Add a checked-in client profile registry with at least one placeholder profile for the first trench
  workflow, for example `trench-variants-1`.
- Resolve the profile in app-shell code and pass the resolved profile or a local error into `Match`
  through constructor options.
- Support an optional profile-owned initial camera view or camera target after normal map bounds are
  applied. A carried camera from a previous match should still win over profile defaults.
- Surface invalid, unsafe, or unknown profile ids as local developer errors without fetching,
  importing, or resolving anything from the id string.
- Add focused contract coverage for URL sanitization, no room-string leakage, registry lookup, and
  invalid-profile behavior.

## Out Of Scope

- No renderer-only samples yet.
- No labels yet.
- No real-unit visual overrides yet.
- No arbitrary asset loading, path loading, dynamic import from URL, hot reload, file picker, upload,
  or remote fetch.
- No server, protocol, checkpoint payload, lab setup submission, minimap, fog, command, balance, or
  simulation changes.

## Expected Touch Points

- `client/src/bootstrap.js`
- `client/src/app.js`
- `client/src/match.js`
- New app-shell profile registry file, classified in `scripts/check-client-architecture.mjs`
- `scripts/check-client-architecture.mjs`
- `tests/client_contracts/launch_url_contracts.mjs`
- New or existing client contract coverage for visual profile registry behavior
- `docs/design/client-ui.md` only if new exported APIs or architecture text need documenting
- `plans/visual-experimentation/phase-1.md` status marker in the implementation commit

## Edge Cases To Cover

- `/lab` with no `visualProfile` keeps the current catalog or direct-launch behavior.
- `/lab?scenario=entrenchment_inspection&visualProfile=trench-variants-1` resolves the checked-in
  profile locally and joins the same server room it would join without the profile id.
- Unsafe profile ids such as path traversal, slashes, dots, URL schemes, SVG text, or overlong input
  fail closed before registry lookup.
- Unknown safe ids do not crash match startup and do not trigger any network or asset fetch.
- A profile camera default applies only when there is no carried camera view.
- Client architecture checks classify any new top-level client file and preserve current lab
  ownership rules.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

Start a local server and open a normal `/lab` route to confirm current catalog/direct-launch behavior
still works. Then open a direct lab URL with `visualProfile=trench-variants-1` and confirm the lab
starts, no server-visible room name contains the profile id, and any profile banner/toast/debug
surface reports the profile cleanly. Open the same URL with an unsafe and an unknown profile id and
confirm the page fails closed locally without breaking the match loop.

## Handoff Expectations

Name the profile registry file, the resolved profile shape, the exact sanitization rule, and how
`App` passes the profile to `Match`. Call out whether Phase 2 should consume a normalized
`profile.staticSamples`, `profile.rendererSamples`, or another final field name.
