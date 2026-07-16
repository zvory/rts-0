# Phase 6 - Report Net Subscriber Failures Safely

Status: Incomplete.

## Objective

Keep `Net` subscriber isolation while making swallowed handler failures visible at low cost. Emit
an always-on console diagnostic for each new stable subscriber-error signature while the fixed
per-instance registry has capacity, optionally mirror it into debug diagnostics, and always
continue to later subscribers.

## Work

- In `Net._emit`, retain the per-handler `try`/`catch` boundary so one broken subscriber cannot stop
  delivery to later subscribers.
- Give each subscribed handler a stable, per-`Net` numeric identifier when it is first registered,
  using a `WeakMap<Function, number>` so diagnostic identity does not retain unsubscribed handlers.
  Define the signature as normalized event type (maximum 64 characters), normalized error name
  (maximum 64), and that local handler identifier. Non-string or missing type/name fields use fixed
  `unknown`/`Error` fallbacks. Do not include `error.message` or `error.stack`, because a subscriber
  may derive either from payload data; do not stringify, inspect, retain, or log the event payload.
- Store reported signatures in a `Set` capped at 32 entries on each `Net` instance. Log only the
  first occurrence while capacity remains. When a 33rd distinct signature arrives, emit one fixed
  saturation report and suppress it and all later new signatures without growing memory; repeated
  recorded signatures remain suppressed. A newly constructed `Net` starts empty; reconnects and
  `disconnect()` do not reset the set, so deduplication semantics remain per instance.
- Emit always-on `console.error("[rts-net] subscriber failure", detail)` for a new signature, where
  `detail` contains only the normalized event type, normalized error name, and handler identifier.
  Emit `console.error("[rts-net] subscriber failure reporting saturated")` once at saturation. If a
  diagnostics collector is present, mirror the same bounded metadata through its existing mark
  seam; the console report must not depend on debug mode being enabled.
- Guard console reporting and the optional diagnostics mirror independently. If either reporter
  throws, continue dispatching the remaining subscribers and do not recurse through `Net._emit`.
- Add focused tests proving identical failures log once, distinct signatures log independently up
  to 32, the 33rd emits only the fixed saturation marker, a second `Net` gets a fresh registry, and
  reconnect/disconnect do not reset it. Throw an error whose message and stack contain a payload
  sentinel and prove neither appears in console or diagnostics metadata; also prove diagnostics
  mirroring is optional and later subscribers run even when the handler and both reporters throw.
  Exercise repeated subscribe/off churn with fresh handlers and prove the only enumerable
  diagnostic registry remains capped at 32 and no strong handler-identity collection is introduced.
- Update the small `Net` contract in `docs/design/client-ui.md` with the bounded reporting behavior.

## Non-goals

- Do not change event ordering, subscription APIs, WebSocket reconnect behavior, or payload
  delivery.
- Do not upload subscriber errors, add telemetry fields, include stack traces or payloads, or build
  a general logging framework.
- Do not alter Lobby, LabClient, SnapshotStreamNet, or other emitters unless focused evidence shows
  they delegate to this exact `Net` seam.
- Do not undertake an exhaustive `Match`/`App` startup transaction or partial-constructor unwind;
  startup rollback is deferred until observed failure evidence justifies that larger change.
- Do not revisit Phase 5 command interaction or control-policy ownership.

## Likely Touch Points

- `client/src/net.js`
- `tests/client_contracts/net_contracts.mjs`
- `docs/design/client-ui.md`

## Verification

- Focused Net contracts for once-per-signature reporting, 32-entry saturation, fresh-instance
  initialization, no reset across disconnect/reconnect, message/stack/payload exclusion, optional
  diagnostics mirroring, subscribe/off churn without strong handler retention, and delivery after
  reporter failure.
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

In a local debug session, temporarily attach one throwing Net subscriber ahead of a healthy one.
Confirm the console reports the bounded failure once, the healthy subscriber continues receiving
events, and repeated identical failures do not spam the console. Remove the temporary subscriber
before committing.

## Handoff

Mark this phase done in its implementation commit. Report the stable payload-independent signature
fields, registry cap and saturation behavior, console prefix/level, optional diagnostics mark, and
evidence that later subscribers still run when reporting fails. Explicitly leave broader
Match/App startup transaction work deferred.
