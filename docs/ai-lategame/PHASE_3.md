# Phase 3 - Unify Expansion Expectations

## Goal

Make every live profile have an eventual expansion path that fits the shared tank late game.

This phase should not make every opening expand at the same time. It should make expansion a shared
late-game expectation instead of a profile-specific accident.

## Plain-Language Intent

If the match goes long, the AI should not stay trapped on one base just because it started with a
rifle opening. While tanks are the only credible late-game power unit, the AI needs enough economy
to keep producing them.

The exact timing can still vary. A fast proxy rush may expand later than a macro rifle profile. A
tank rush may expand after tank tech is stable. But all live profiles should eventually have a path
to a second City Centre.

## Current State

`tech_to_tanks` already has an expansion policy:

- target City Centres: 2;
- prerequisite: completed Factory;
- trigger: 500 steel or 70 supply;
- does not block tech path.

`rifle_flood_full_saturation` already has an expansion policy:

- target City Centres: 2;
- prerequisite: completed Training Centre;
- trigger: 500 steel or 50 supply;
- does not block tech path.

`rifle_flood_fast` already has an expansion policy:

- target City Centres: 2;
- prerequisite: completed Factory;
- trigger: 500 steel or 70 supply;
- does not block tech path.

The problem is not that expansion is entirely missing. The problem is that late-game expansion
expectations are embedded separately in each profile and are easy to drift.

## Proposed Shape

Introduce a shared live late-game expansion expectation, then let profiles choose timing gates where
needed.

Shared expectation:

- eventual target: 2 City Centres;
- expansion does not block tank tech for live profiles;
- expansion should be allowed once the profile has reached a stable tech/economy point;
- post-expansion worker caps should support sustained tank+rifle production.

Profile-specific timing can remain:

- `tech_to_tanks`: expand after Factory is complete or when late-game tank plan is stable;
- `rifle_flood_full_saturation`: expand earlier, around Training Centre or 50 supply, because it has
  a stronger worker base;
- `rifle_flood_fast`: keep a conservative expansion gate until a separate recovery plan exists.

## Implementation Options

### Option A: Shared Expansion Constants

Keep `ExpansionPolicy` where it is, but define shared constants for repeated values:

- target City Centre count;
- post-expansion steel worker cap;
- search radius;
- expansion blocking behavior.

Use profile-local values only where timing should differ.

This is the least invasive option and probably enough for now.

### Option B: Shared Late-Game Expansion Helper

Add a helper or constructor that creates expansion policies from a small set of knobs:

- required completed building;
- trigger steel;
- trigger supply;
- pre-expansion worker cap.

This reduces duplication and makes intent clearer, but may be more abstraction than needed.

### Option C: Full Shared Late-Game Policy Object

Fold production, attack, tech path, and expansion into one shared late-game policy object.

This is clean conceptually, but it starts to resemble profile phases. Avoid it unless Phase 2 leaves
the profile data awkward.

## Recommendation

Start with Option A. Use shared constants for the common expansion expectation and leave the timing
knobs profile-specific.

This keeps the code close to the current model while still preventing silent drift.

## Interaction With `rifle_flood_fast`

Do not tune around the proxy profile's current economic weakness in this phase.

It is acceptable for `rifle_flood_fast` to have an expansion policy that rarely fires until its
separate recovery plan exists. The purpose here is to ensure the profile has a coherent late-game
destination, not to guarantee that the proxy opening reaches it often.

## Expected Behavior At End

At the end:

- every live profile has an eventual second City Centre expansion policy;
- shared late-game expansion assumptions are named in one place;
- profile-specific timing remains possible;
- no live profile is intentionally one-base forever;
- tank late-game production is economically supported when the AI survives long enough.

## Tests

Focused tests should prove:

- every profile in the live profile pool has an expansion policy;
- every live profile's expansion target is at least 2 City Centres;
- live-profile expansions do not block the tank tech path unless explicitly intended;
- the shared expansion constants are applied consistently;
- `tech_to_tanks` still expands after reaching its existing tank-tech gate.

Long self-play can be useful after this phase to catch economic stalls, but unit tests should cover
the policy-level guarantee.

## Done When

- Expansion expectations are visibly shared.
- Live profiles still have distinct opening timings.
- The docs and tests agree that expansion is an eventual late-game expectation for all live AI.
