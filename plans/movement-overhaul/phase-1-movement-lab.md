# Phase 1: Build the Movement Lab

## Purpose

Create a small, repeatable place to watch movement problems happen. This should be easier to use
than a full match and more trustworthy than staring at test output.

The movement lab should make movement visible, controllable, and debuggable.

## Core Scenarios

Start with a short list. Do not make a giant scenario suite yet.

- one tank crossing open ground;
- one tank turning around a building corner;
- one scout car turning around the same corner;
- one AT gun turning around the same corner;
- one vehicle through a snaking corridor;
- two vehicles meeting in a two-wide choke;
- ten vehicles moving through a two-wide choke;
- one vehicle starting too close to a wall and recovering.

## Viewer Needs

The viewer should show the real server simulation, not a separate fake model.

It should support:

- play and pause;
- stepping one tick at a time;
- jumping to a specific tick;
- selecting a unit;
- showing body hulls;
- showing intended route or target;
- showing why a unit is blocked or waiting.

## Trace Needs

For selected units, the viewer should expose plain decision notes:

- what the unit was trying to do;
- what choices were considered;
- which choice was picked;
- which choices were rejected;
- the reason for each rejection.

The trace should answer questions like: "Why did this tank turn into the wall on tick 418?"

## Done

- A developer can open each core scenario locally.
- The scenario can be watched without playing a full match.
- A selected unit has enough visible state to explain obvious failures.
- The scenario setup is repeatable.
