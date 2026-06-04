# Phase 4 - Local Steering Alignment

Goal: align future local steering with the new standability and footing model.

This phase corresponds to `docs/movement/PHASE_6_LOCAL_STEERING.md`. It should only be implemented
once the lower-level correctness phases have landed.

## Dependencies

- Collision Phases 0-3.
- Movement Phase 6 local steering design.

## Scope

In scope:

- Use shared footing/solidness helpers for steering neighbor weights.
- Use standability for steering candidate landings.
- Keep collision resolution as the final authority after steering.
- Add tests that steering reduces avoidable overlap without bypassing static blockers.

Out of scope:

- No ORCA/RVO.
- No flow fields.
- No global dynamic path costs.
- No persistent formation slots.
- No changes to commands or wire protocol.

## Design

Local steering is a proposal layer:

```text
path or tank-body desired direction
+ short-range separation from solid nearby units
= candidate direction
```

Standability remains the authority:

```text
candidate position must pass unit_static_standable
```

Collision remains the cleanup:

```text
after movement, resolve remaining unit-unit overlap deterministically
```

Do not let steering become another hidden legality system.

## Shared Footing

Movement Phase 0 currently keeps `FootingProfile` local to `movement.rs`. Once steering and
collision both need it, extract or expose a pure helper:

```rust
pub(crate) enum FootingProfile {
    Ghost,
    Soft,
    Firm,
    Braced,
    Heavy,
}

pub(crate) fn footing_profile(e: &Entity) -> FootingProfile;
pub(crate) fn footing_resistance(profile: FootingProfile) -> f32;
pub(crate) fn is_pass_through(profile: FootingProfile) -> bool;
```

Keep profile classification deterministic and free of spatial queries.

## Steering Rules

Steering should:

- ignore self,
- ignore dead entities,
- ignore non-units,
- ignore ghost/pass-through units,
- cap neighbors deterministically after sorting ids,
- weight separation more strongly near overlap,
- weight firm/braced/heavy neighbors more strongly than soft neighbors,
- validate final candidate position through standability,
- fall back to existing movement if the steered candidate is blocked.

Steering should not:

- reserve tiles,
- change production spawn behavior,
- change construction placement behavior,
- make units tunnel through buildings,
- hide illegal overlap that collision should report.

## Tests

Add or adapt tests from movement Phase 6:

- `moving_unit_steers_around_braced_unit_when_space_exists`
- `choke_still_clogs_when_no_space_exists`
- `steering_ignores_ghost_harvester`
- `steering_candidate_rejected_when_body_would_clip_building`
- `steering_neighbor_cap_is_deterministic`
- Existing collision and tank locomotion tests still pass.

Run:

```bash
cd server && cargo fmt && cargo test movement::tests standability
cd server && cargo test
```

## Acceptance Criteria

- Steering reduces avoidable overlap pressure.
- Steering never bypasses static standability.
- Chokes still clog when there is no real space.
- The same footing model drives steering and hard collision.
- Collision remains active after steering.
