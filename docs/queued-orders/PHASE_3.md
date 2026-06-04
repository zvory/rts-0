# Phase 3 - Queued Worker Build and Gather Handoff

Goal: support practical worker chains such as "build this, then go back to steel."

## Scope

- Allow queued worker intents for:
  - build
  - gather
  - move
- Build completion:
  - Queued orders after `Build` run only after construction completes.
  - If the build intent fails before construction starts, silently skip to the next queued order.
  - If the building is destroyed while under construction, define that as build completion failure
    for queue promotion and silently try the next queued order.
- Gather semantics:
  - Gather is terminal infinite once harvesting starts.
  - A queued gather intent validates the node, remaining resources, worker ownership, worker kind,
    and completed Industrial Center requirement when promoted.
  - If invalid at promotion, silently skip to the next queued order.
- Stop/replacement:
  - Normal gather/build commands clear existing queued orders.
  - `Stop` clears queued worker handoff orders unless the worker is already in non-interruptible
    construction, matching current construction constraints.

## Design Notes

Do not make construction interruptible as part of this phase. The existing rule that constructing
workers cannot be pulled away is a gameplay and simulation invariant. Queue promotion should happen
from the construction system when the worker is released.

Gather should not try to represent "mine one trip then continue" in this phase. That would be a
different order kind with explicit finite gather semantics.

## Tests

- Worker builds a depot and then moves to a queued destination after construction completes.
- Worker builds and then gathers a queued steel node after construction completes.
- Queued gather on a depleted node is skipped without a panic or notice.
- Build placement failure skips to the next queued worker order.
- Construction destruction releases the worker and skips/promotes according to the queue.
- A gathering worker does not continue to later queued orders after harvesting starts.

## Done

- The common worker handoff flow works: Shift-build, Shift-gather back to steel.
- Worker queue behavior remains compatible with current construction non-interruptibility.

