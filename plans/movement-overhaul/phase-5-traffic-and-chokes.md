# Phase 5: Traffic and Chokes

## Purpose

Add group movement only after single vehicles are reliable.

Traffic should be explicit and readable. Units should not solve traffic by randomly steering away
from each other into walls.

## Starting Model

Begin with simple rules:

- each unit proposes a movement choice;
- choices that collide with terrain are rejected;
- choices that collide with other units are rejected or delayed;
- a deterministic priority rule decides who goes first;
- lower-priority units wait, slow down, or reverse when blocked.

The first version can be conservative. It is better for a unit to wait clearly than to make a
mysterious bad turn.

## Core Choke Scenarios

Use the lab to validate:

- two tanks meeting head-on in a two-wide choke;
- ten tanks moving through a two-wide choke;
- mixed scout cars and tanks entering a choke;
- AT guns moving with other vehicles;
- vehicles turning around each other near a corner.

## What To Avoid

Avoid making repulsion or steering pressure the core traffic model. It can be added later as polish,
but it should not be the thing that keeps units legal.

## Done

- Units do not drive into walls to avoid each other.
- Units can wait without being considered broken.
- Chokes eventually drain in the core scenarios.
- Traffic decisions have readable reasons.
- Scenario tests include broad timing limits so traffic cannot silently get much worse.
