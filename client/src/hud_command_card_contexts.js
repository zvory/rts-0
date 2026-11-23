import { CULTIVATORS_FACTION_ID, EKAT_FACTION_ID } from "./config.js";
import { ABILITY, KIND, UPGRADE } from "./protocol.js";

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

/** Cultivator commands must be registered for both preset and custom hotkey profiles. */
export function buildCultivatorCommandCardContextSamples(buildCard) {
  const disciple = { id: 20, owner: 1, kind: KIND.WARRIOR, insight: 60 };
  const worker = { id: 21, owner: 1, kind: KIND.WORKER };
  const ctx = (selection) => ({
    playerId: 1, factionId: CULTIVATORS_FACTION_ID, selection,
    entities: selection, resources: { steel: 1000, oil: 1000 },
    upgrades: [UPGRADE.DAO_OF_LIGHTNESS, UPGRADE.DAO_OF_HEAVINESS, UPGRADE.DAO_OF_MAGIC],
    groupCooldownClocks: () => [], playerHasCompleteKind: () => true,
  });
  const unitCards = [null, "lightness", "war", "magic"].map((dao) => ({
    id: `cultivators-disciple-${dao || "base"}`,
    card: buildCard(ctx([{ ...disciple, dao, kind: ({ lightness: KIND.LIGHTNESS_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao] || KIND.WARRIOR }])),
  }));
  return [
    ...unitCards,
    { id: "cultivators-shop", card: buildCard(ctx([{ id: 30, owner: 1, kind: KIND.SHOP }])) },
    { id: "cultivators-worker", card: buildCard(ctx([worker])) },
    { id: "cultivators-build", card: buildCard({ ...ctx([worker]), commandCardMode: "workerBuild" }) },
    ...[KIND.RESOURCE_DEPOT, KIND.PORTAL, KIND.THREE_DAO_PAVILION, KIND.TOURNAMENT_GROUNDS, KIND.PROVEN_GROUNDS].map((kind, i) => ({
      id: `cultivators-producer-${kind}`,
      card: buildCard({ ...ctx([{ id: 22 + i, owner: 1, kind }]), upgrades: [] }),
    })),
  ];
}
