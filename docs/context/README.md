# Context capsules

Small, task-scoped pointers into `DESIGN.md` and the code. Read the capsule that matches your task
instead of paying the full `DESIGN.md` token cost up front.

Capsules are pointers, not copies. `DESIGN.md` remains the source of truth — when a capsule and
`DESIGN.md` disagree, `DESIGN.md` wins. Update `DESIGN.md` first, then refresh the capsule's
section list if structure changed.

| Task                                                | Capsule                                |
| --------------------------------------------------- | -------------------------------------- |
| Simulation, tick, services, AI, self-play harness   | [server-sim.md](server-sim.md)         |
| Rendering, input, HUD, client modules, teardown     | [client-ui.md](client-ui.md)           |
| Wire messages, snapshot shape, fog filtering        | [protocol.md](protocol.md)             |
| Costs, supply, sight, unit/building stats           | [balance.md](balance.md)               |
| Node integration tests, regression, client smoke    | [testing.md](testing.md)               |
| Hardening limits, server bind, build/run pipeline   | [deployment.md](deployment.md)         |

Read `DESIGN.md` in full only when changing cross-file contracts (protocol ⇄ client, `Game` API,
balance mirror, fog rules). Otherwise, the capsule + the code is enough.
