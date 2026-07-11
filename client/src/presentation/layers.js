export const PRESENTATION_LAYER_DESCRIPTORS = Object.freeze([
  Object.freeze({ id: "staticGround", order: 0, space: "world", visibilityPolicy: "static", depthPolicy: "ground" }),
  Object.freeze({ id: "persistentGroundMark", order: 1, space: "world", visibilityPolicy: "alreadyFiltered", depthPolicy: "ground" }),
  Object.freeze({ id: "fogGatedWorld", order: 2, space: "world", visibilityPolicy: "alreadyFiltered", depthPolicy: "world" }),
  Object.freeze({ id: "rememberedWorld", order: 3, space: "world", visibilityPolicy: "remembered", depthPolicy: "world" }),
  Object.freeze({ id: "belowFogIntel", order: 4, space: "world", visibilityPolicy: "intel", depthPolicy: "world" }),
  Object.freeze({ id: "currentFog", order: 5, space: "world", visibilityPolicy: "fogMask", depthPolicy: "overlay" }),
  Object.freeze({ id: "aboveFogReveal", order: 6, space: "world", visibilityPolicy: "reveal", depthPolicy: "world" }),
  Object.freeze({ id: "tacticalFeedback", order: 7, space: "world", visibilityPolicy: "local", depthPolicy: "overlay" }),
  Object.freeze({ id: "screenOverlay", order: 8, space: "screen", visibilityPolicy: "local", depthPolicy: "screen" }),
]);

export const PRESENTATION_LAYER_IDS = Object.freeze(
  PRESENTATION_LAYER_DESCRIPTORS.map((descriptor) => descriptor.id),
);

export function createEmptyLayerRecords() {
  return Object.fromEntries(PRESENTATION_LAYER_IDS.map((id) => [id, []]));
}
