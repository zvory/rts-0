# Context capsules

Small, task-scoped pointers into `docs/design/` and the code. Read the capsule that matches your
task instead of paying the full design-doc token cost up front.

Capsules are pointers, not copies. `docs/design/*.md` are the source of truth by contract area. If
a capsule and a design doc disagree, the design doc wins. Update the relevant design doc first,
then refresh the capsule's section list if structure changed.

| Task                                                | Capsule                                |
| --------------------------------------------------- | -------------------------------------- |
| Simulation, tick, services, AI, self-play harness   | [server-sim.md](server-sim.md)         |
| Rendering, input, HUD, client modules, teardown     | [client-ui.md](client-ui.md)           |
| Wire messages, snapshot shape, fog filtering        | [protocol.md](protocol.md)             |
| Costs, supply, sight, unit/building stats           | [balance.md](balance.md)               |
| Node integration tests, regression, client smoke    | [testing.md](testing.md)               |
| Hardening limits, server bind, build/run pipeline   | [deployment.md](deployment.md)         |
| Match history persistence and `/api/matches`        | [match-history.md](match-history.md)   |

Read the relevant design doc only when changing cross-file contracts (protocol ⇄ client,
`Game` API, balance mirror, fog rules). Otherwise, the capsule + the code is enough.

For new unit work, start with [docs/new-unit-checklist.md](../new-unit-checklist.md), then read the
task-specific capsules it references as each phase begins.
