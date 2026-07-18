import { EKAT_FACTION_ID } from "./config.js";
import { ABILITY, KIND } from "./protocol.js";

/** Representative rendered Ekat cards for hotkey-profile validation. */
export function buildEkatCommandCardContextSamples(buildCard) {
  const playerId = 1;
  const ekat = {
    id: 17,
    owner: playerId,
    kind: KIND.EKAT,
    abilities: [
      ABILITY.EKAT_TELEPORT,
      ABILITY.EKAT_LINE_SHOT,
      ABILITY.EKAT_MAGIC_ANCHOR,
      ABILITY.EKAT_CONSUME_GOLEM,
    ].map((ability) => ({ ability, cooldownLeft: 0, remainingUses: null })),
  };
  const golem = { id: 18, owner: playerId, kind: KIND.GOLEM };
  const zamok = { id: 19, owner: playerId, kind: KIND.ZAMOK, buildProgress: null };
  const ctx = (selection) => ({
    playerId,
    factionId: EKAT_FACTION_ID,
    selection,
    entities: [ekat, golem, zamok],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    groupCooldownClocks: () => [],
    playerHasCompleteKind: () => true,
  });
  return [
    { id: "ekat-unit", card: buildCard(ctx([ekat])) },
    { id: "ekat-zamok-train", card: buildCard(ctx([zamok])) },
  ];
}
