# Phase 2: Scenario Setup API

## Objective

Clean up the setup code introduced in Phase 1 so scenarios and tests can author exact worlds without
copying private fixtures or exposing debug powers to clients.

## Setup Capabilities

Add `pub(crate)` helpers for server-side scenario/test code only:

- create a `Game` from a custom `Map`;
- choose explicit `PlayerInit` records;
- disable normal starting bases/resources;
- spawn units, buildings, and resource nodes at exact world positions;
- set unit facing and initial orders;
- recompute spatial index, supply, and fog after setup;
- return enough setup failure context to make scenario registration errors obvious at startup.

Do not expose these helpers through client commands, WebSocket messages, or normal gameplay APIs.

## Scout-Car Fixture Extraction

Move the scout-car snaking corridor constants/builders out of
`server/src/game/services/movement/tests.rs` into a server-side helper that both the ignored timing
test and the dev scenario can use.

The shared fixture should preserve:

- corridor geometry;
- scout-car spawn layout for `cars=1` and `cars=4`;
- goal point;
- exit-clear threshold used by the timing test;
- helper state descriptions useful for debugging.

## Done

- The Phase 1 scenario no longer owns a one-off setup path.
- The ignored timing test and scenario share the same fixture source.
- Setup helpers remain `pub(crate)` and are reachable only from server-side code.

## Verification

- `cd server && cargo test`
- Run the ignored timing scenario manually when changing fixture behavior:

```text
cargo test scout_car_snaking_corridor_clear_times -- --ignored --nocapture
```

- Manually open the dev scenario after fixture extraction to confirm the rendered setup still
  matches the timing test.
