# Context capsules

Small, task-scoped pointers into `docs/design/` and the code. Read the capsule that matches your
task instead of paying the full design-doc token cost up front.

Capsules are pointers, not copies. `docs/design/*.md` are the source of truth by contract area. If
a capsule and a design doc disagree, the design doc wins. Update the relevant design doc first,
then refresh the capsule's section list if structure changed.

| Task                                                | Capsule                                |
| --------------------------------------------------- | -------------------------------------- |
| Simulation, tick, services, AI, self-play harness   | [server-sim.md](server-sim.md)         |
| Plain-language server architecture walkthrough      | [../design/server-architecture-walkthrough.md](../design/server-architecture-walkthrough.md) |
| Rendering, camera/projection, input, HUD, teardown  | [client-ui.md](client-ui.md)           |
| Wire messages, snapshot shape, fog filtering        | [protocol.md](protocol.md)             |
| Costs, supply, sight, unit/building stats           | [balance.md](balance.md)               |
| Node integration tests, regression, client smoke    | [testing.md](testing.md)               |
| Hardening limits, server bind, build/run pipeline   | [deployment.md](deployment.md)         |
| Server wiki route and generated stats reference     | [deployment.md](deployment.md)         |
| Match history persistence and `/api/matches`        | [match-history.md](match-history.md)   |
| Multi-phase or phased implementation planning       | [planning.md](planning.md)             |
| Hotspot analysis methodology and group tracking     | [../hotspot-analysis.md](../hotspot-analysis.md) |

Read the relevant design doc only when changing cross-file contracts (protocol ⇄ client,
`Game` API, balance mirror, fog rules). Otherwise, the capsule + the code is enough.

For new unit work, start with [docs/new-unit-checklist.md](../new-unit-checklist.md), then read the
task-specific capsules it references as each phase begins.

For new building work, start with [docs/new-building-checklist.md](../new-building-checklist.md),
then read the task-specific capsules it references as each phase begins. For faction work that adds
both units and buildings, use both checklists and keep the Phase 0/1 user-review gate explicit.

For any multi-phase or phased implementation plan, read [planning.md](planning.md) before writing
the plan.

For hotspot scoring, architectural group tracking, and before/after cleanup comparisons, use
[docs/hotspot-analysis.md](../hotspot-analysis.md). Create a plan only after current hotspot
evidence supports concrete implementation phases.

For optional source-to-doc navigation, use the advisory routing map at
[../doc-map.json](../doc-map.json). It points common source areas to likely relevant capsules and
design docs, but it does not claim exclusive ownership or require a docs change for every mapped
source change.
