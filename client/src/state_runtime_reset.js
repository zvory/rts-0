import { GroundDecalBuffer } from "./state_ground_decals.js";
import { VisualEffectBuffers } from "./state_visual_effects.js";

/** Clear all state derived from a particular authoritative timeline. */
export function resetAuthoritativeRuntime(state) {
  state._prev = null;
  state._cur = null;
  state._prevRecvTime = 0;
  state._curRecvTime = 0;
  state._prevById = new Map();
  state._curById = new Map();
  state.resources = { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
  state.playerResources = [];
  state.events = [];
  state.upgrades = [];
  state.selection.clear();
  state.selectionBudgetOverflow = null;
  state.controlGroups = Array.from({ length: 10 }, () => []);
  state.smokes = [];
  state.abilityObjects = [];
  state.trenches = [];
  state.rememberedBuildings = [];
  state.rememberedAntiTankGuns = [];
  state.visibleTiles = [];
  state.exploredTiles = [];
  state.groundDecals = new GroundDecalBuffer();
  state.visualEffects = new VisualEffectBuffers();
  state.predictionPatchById.clear();
  state.predictionProgressById.clear();
  state.predictionCorrectionById.clear();
  state.predictionDiagnostics = null;
  state.predictionProgressPaused = false;
  state.optimisticProduction = [];
  state.optimisticProductionByBuilding.clear();
  state.optimisticRallyByBuilding.clear();
}
