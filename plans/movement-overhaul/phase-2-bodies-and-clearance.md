# Phase 2: Prove Static Bodies and Clearance

## Purpose

Before rebuilding movement, prove that the game can answer the most basic physical questions:

- Is this vehicle pose legal?
- Does this body overlap a wall or building?
- Does this motion sweep through a wall?
- How much clearance does this vehicle have near a corner?

If these answers are wrong, no movement planner can be trusted.

## Approach

Use the movement lab to test body and clearance behavior without traffic, combat, economy, or AI.
Place vehicles at known positions and facings near walls, corners, and corridors. Show the body and
clearance visually.

This is the first salvage checkpoint. If existing body and legality helpers give correct,
understandable answers, keep them. If they are confusing or wrong, replace them before moving on.

## What To Prove

- Tanks, scout cars, and AT guns have visible bodies that match gameplay expectations.
- A legal pose never visually clips into terrain or buildings.
- An illegal pose is rejected for a clear reason.
- A swept motion is rejected if the vehicle would pass through a wall.
- Corner clearance can be seen and measured.
- The same clearance concept applies to tanks, scout cars, and AT guns.

## Done

- Static body checks are trusted or replaced.
- Clearance around corners is visible in the lab.
- The team can say which geometry pieces are safe to salvage.
- No traffic behavior has been added yet.
