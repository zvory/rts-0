# Phase 4: Single-Vehicle Movement

## Purpose

Make one vehicle move well before solving groups and traffic.

If a single vehicle cannot handle open ground, a corner, and a corridor, group movement will only
hide the real problems.

## Tank First

Start with tanks because they can rotate in place. A simple tank should be able to:

- face its target;
- rotate at a believable rate;
- move forward when aligned;
- reverse when that is the sensible physical choice;
- refuse illegal poses;
- leave space around corners.

## Scout Car Next

Scout cars should reuse the same body and clearance rules, but their motion choices are different.
They should not rotate in place.

A simple scout car can start with:

- forward straight;
- forward left arc;
- forward right arc;
- reverse straight;
- waiting when no legal move exists.

Reverse arcs and smarter recovery can come later.

## AT Gun After That

AT guns should share the same clearance model. Their exact movement feel can be tuned after tanks
and scout cars are understandable.

The important question is whether they behave like a slow, awkward vehicle with correct clearance,
not whether every final number is perfect.

## Done

- A tank clears the open-ground, corner, and corridor scenarios.
- A scout car clears the same scenarios without rotating in place.
- An AT gun clears the same scenarios with shared clearance behavior.
- Each movement decision can be explained in the viewer.
- No group traffic behavior is required yet.
