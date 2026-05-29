# TODO

## Active, In Order

 - [x] Replace stringly entity kind checks in hot simulation paths with typed internal enums, converting to protocol strings only at the boundary.
 - [x] Split `systems.rs` into internal services before adding complex mechanics: commands, movement, combat, economy, production, construction, death, occupancy.
 - [x] Add a spatial query layer used by combat, fog, resource search, collision/steering, and snapshot interest filtering.
  - [x] Introduce a `PathingService` boundary with unit class, radius/footprint, terrain mask, dynamic blockers, path budget, and cached/reusable results.
 - [x] Extend map/passability around movement classes before terrain-specific combat and tank/infantry rules land.
 - [x] Rename the game officially to Bewegungskrieg.
 - [ ] Restyle the main menu to be more ww2 themed, less scifi.
 - [x] Enforce map-generation resource fairness: Industrial Centers must keep a minimum distance from minerals and gas, and spawn layouts should precisely control resource distances so no player gets an advantage from patches or geysers being too close or too far.
 - [ ] Units should have collision and not stack, unless they're mining workers. This is a tough change because it requires complex pathfinding and careful thought and modular design.
 - [ ] Maps should be twice as large.
 - [ ] Implement forests: LoS blockers unless you're inside them, provide cover (attacks on them have chance to miss), tanks cannot enter but can shoot into them. Forests should come in large blobs and block vision. Infantry attacking from inside are visible.
 - [ ] Buildings should be able to rally units to a position.
 - [ ] AI should GG and leaveafter losing all town halls.
 - [ ] Machine gunner attacks should be the same as infantryman unless the command "set up" is used. Setup stops the machine gunner for five seconds with no moving or shooting; after setup they cannot move or rotate without tearing down for three seconds, but have elevated damage output and a fixed 40 degree field of fire.
 - [ ] Support two different factions, soviets and germans, with different stats and some special units and buildings. This one requires engineer inptu to design.
- [x] Rename gas to oil, minerals to steel.


## Needs prioritization
 - [ ] lobby system to see active lobbies
 - [ ] All data should exist as TOML files, not hardcoded 
 - [ ] Hotkeys in the command card should be grid style, as in the top left hotkey is always Q, the top middle is W, the one below the top left is A, etc. Match the keyboard.
 - [ ] workers should not auto attack
 - [ ] Evolve snapshots toward baseline + delta updates or entity dirty flags while preserving the current `snapshot_for(player)` API.
 - [ ] muzzle flare animations
 - [ ] basic settings menu or something for surrendering
 - [ ] client should display latency to server in the top left in miliseconds
 - [ ] find a source of copyright free assets we can use for units, buildings, resources, and use it
 - [ ] implement a correct system for building buildings, currently a worker pulled away from building a building will stop construciton and the building will be permanently unbuildable, resumption is impossible
 - [ ] switch font to DIN 1451 Mittelschrift everywhere
 - [ ] display "connectino to server lost" when connection to server lost
 - [ ] AI should attack once with riflemen, then eco/tech up to attack with a machine gunner supported by riflemen, then eco/tech up to attack with a tank supported by riflemen and machine gunners

## Done

 - [x] Add a tick-stamped command log and deterministic replay harness as a first-class internal artifact.
 - [x] Command card should have grid style hotkeys.
 - [x] Feedback on move and attack commands.
 - [x] Selected units should have a command card for hold position, move, attack, stop.
 - [x] Progress bars on top of buildings for their unit building progress.
 - [x] Defeat screen should not black out the screen, but continue to show the latest state of the map.
 - [x] Command card should be made bigger.
 - [x] All units besides rifleman and workers should require gas.
 - [x] Halve the speed of infantry.
 - [x] Unpassable terrain should have a darker silhouette.
 - [x] Tanks should be twice the size they are.
 - [x] Halve the speed of a worker and AT gun too.
 - [x] Fix workers getting stuck inside buildings after building them.
