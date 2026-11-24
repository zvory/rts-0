import { assert } from "./assertions.mjs";

export function assertMatchStopClearsSwordAudio(Match) {
  const match = Object.create(Match.prototype);
  let swordStops = 0;
  match.running = true;
  match.closeMenus = () => {};
  match.combatAudio = {
    updateThrowSwordSounds(objects = []) {
      assert(objects.length === 0, "stopping a match clears sustained sword voices");
      swordStops += 1;
    },
  };
  match.stop();
  assert(!match.running && swordStops === 1,
    "the live game-over stop silences swords without waiting for another snapshot");
  match.stop();
  assert(swordStops === 2, "repeated match stops safely clear sword audio");
}
