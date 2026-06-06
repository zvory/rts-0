# Phase 3: Create the New Movement System

## Purpose

Create a new movement system beside the old one. It should use the same outer simulation hook, but
its internal behavior should be simple and fresh.

This lets the team rebuild movement without first untangling the legacy system.

## Shape

The new system can be called `movement_v2` or another obvious temporary name. It should be easy to
switch scenarios between old movement and new movement.

At first, the new system should do very little:

- read the unit's current route or goal;
- generate a small set of possible motions;
- reject illegal motions;
- pick one legal motion;
- move the unit;
- log the decision.

## What Not To Add Yet

Do not add the old rescue behaviors at the start:

- no waypoint skipping magic;
- no wall slide;
- no sidestep;
- no local repulsion steering;
- no traffic bias;
- no special unjam behavior;
- no clever recovery unless it is a named movement choice.

If a unit cannot move, it should wait or report that no legal motion was found.

## Done

- The new movement system can run in at least one lab scenario.
- The old movement system still exists.
- The new system logs simple decisions.
- The system is intentionally boring and easy to reason about.
