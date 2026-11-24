import { assert } from "./assertions.mjs";

export function assertMatchStopClearsSwordAudio(Match) {
  const match = Object.create(Match.prototype);
  let swordStops = 0, abilityStops = 0;
  match.running = true;
  match.closeMenus = () => {};
  match.combatAudio = {
    abilityAudio: { stop({ reset }) { assert(reset, "match stop resets ability timeline"); abilityStops += 1; } },
    updateThrowSwordSounds(objects = []) {
      assert(objects.length === 0, "stopping a match clears sustained sword voices");
      swordStops += 1;
    },
  };
  match.stop();
  assert(!match.running && swordStops === 1,
    "the live game-over stop silences swords without waiting for another snapshot");
  match.stop();
  assert(abilityStops === 2, "match stops clear sustained anchors too");
  assert(swordStops === 2, "repeated match stops safely clear sword audio");
}
