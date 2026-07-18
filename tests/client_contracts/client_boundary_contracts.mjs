// tests/client_contracts/client_boundary_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertDeepEqual,
} from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { HUD } from "../../client/src/hud.js";
import {
  ABILITY,
  KIND,
} from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { ClientIntent } from "../../client/src/client_intent.js";

// ---------------------------------------------------------------------------
// Client boundary baseline contracts
// ---------------------------------------------------------------------------

{
  const intent = new ClientIntent({ now: () => 100 });
  intent.openWorkerBuildMenu();
  assert(intent.commandCardMode === "workerBuild", "ClientIntent owns command-card submenu state");
  intent.beginPlacement(KIND.DEPOT);
  assertDeepEqual(intent.placement, {
    building: KIND.DEPOT,
    tileX: 0,
    tileY: 0,
    valid: false,
  }, "ClientIntent seeds placement previews");
  intent.updatePlacement(3, 4, true);
  assertDeepEqual(intent.placement, {
    building: KIND.DEPOT,
    tileX: 3,
    tileY: 4,
    valid: true,
  }, "ClientIntent updates placement previews");
  intent.beginCommandTarget("attack", { now: 100, shiftKey: true });
  assert(intent.placement === null, "ClientIntent clears placement when targeting begins");
  assert(intent.commandTarget === "attack", "ClientIntent mirrors armed command targets");
  intent.updateAttackTargetPreview({ targetId: 300, kind: KIND.RIFLEMAN, x: 80, y: 96 });
  intent.updateAntiTankGunSetupPreview({ mouseX: 1, mouseY: 2, guns: [] });
  intent.updateAbilityTargetPreview({ ability: ABILITY.SMOKE, carriers: [], hoverInRange: true });
  intent.beginCommandTarget("move", { now: 200 });
  assert(intent.attackTargetPreview === null, "ClientIntent clears attack hover previews on target changes");
  assert(intent.antiTankGunSetupPreview === null, "ClientIntent clears support previews on target changes");
  assert(intent.abilityTargetPreview === null, "ClientIntent clears ability previews on target changes");
  intent.addCommandFeedback("move", 10, 20, true, null, 100);
  assert(intent.liveCommandFeedback(700).length === 1, "ClientIntent keeps fresh command feedback");
  assert(intent.liveCommandFeedback(751).length === 0, "ClientIntent expires command feedback by TTL");
  intent.updateResourceMiningPreview({
    resourceId: 200,
    resourceX: 64,
    resourceY: 96,
    ccId: 3,
    ccX: 48,
    ccY: 48,
    inRange: true,
  });
  assert(intent.resourceMiningPreview?.resourceId === 200, "ClientIntent owns resource hover previews");
  intent.updateAttackTargetPreview({ targetId: 301, kind: KIND.RIFLEMAN, x: 96, y: 96 });
  assert(intent.attackTargetPreview?.targetId === 301, "ClientIntent owns attack hover previews");
  const armedLabTool = intent.beginLabTool({ kind: "fieldPoint", payload: { xField: "spawn-x", yField: "spawn-y" } });
  assert(armedLabTool.id && armedLabTool.kind === "fieldPoint", "ClientIntent arms lab tools with stable ids");
  intent.updateLabToolPreview({ toolId: armedLabTool.id, x: 12, y: 24 });
  const updatedLabTool = intent.updateLabToolPayload({ xField: "target-x", owner: 2 });
  assert(updatedLabTool === armedLabTool, "ClientIntent updates an active Lab tool without replacing its identity");
  assertDeepEqual(
    intent.labToolPreview,
    {
      toolId: armedLabTool.id,
      kind: "fieldPoint",
      x: 12,
      y: 24,
      payload: { xField: "target-x", owner: 2 },
    },
    "ClientIntent immediately updates the payload of an existing Lab tool preview",
  );
  assert(intent.commandTarget === null && intent.placement === null, "ClientIntent lab tools clear placement and command targeting");
  assert(intent.resourceMiningPreview === null, "ClientIntent lab tools clear hover previews");
  assert(intent.attackTargetPreview === null, "ClientIntent lab tools clear attack hover previews");
  intent.beginPlacement(KIND.DEPOT);
  assert(intent.activeLabTool === null, "ClientIntent placement cancels active lab tools");
  intent.beginLabTool({ kind: "fieldPoint" });
  intent.beginCommandTarget("move");
  assert(intent.activeLabTool === null, "ClientIntent command targeting cancels active lab tools");
  intent.beginLabTool({ kind: "fieldPoint" });
  assert(intent.cancelLabTool("escape")?.reason === "escape", "ClientIntent reports lab tool cancellation reason");
  assert(intent.activeLabTool === null, "ClientIntent clears lab tool cancellation state");

  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 12, height: 12, tileSize: 32, terrain: new Array(144).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  assert(!("clientIntent" in state), "GameState no longer owns browser-local intent state");
  assert(!("placement" in state), "GameState no longer exposes placement intent shims");
  assert(!("commandTarget" in state), "GameState no longer exposes command-target intent shims");
  assert(!("activeLabTool" in state), "GameState no longer exposes lab-tool intent shims");
  assert(!("attackTargetPreview" in state), "GameState no longer exposes attack hover preview shims");

  const explicitHudIntent = new ClientIntent();
  const facadeHud = Object.create(HUD.prototype);
  facadeHud.state = {
    beginPlacement() {
      throw new Error("HUD must use injected ClientIntent for placement");
    },
    beginCommandTarget() {
      throw new Error("HUD must use injected ClientIntent for command targeting");
    },
  };
  facadeHud.clientIntent = explicitHudIntent;
  facadeHud.commandInteraction = { issueCommand() {} };
  facadeHud._dispatchCommandIntent({ type: "openWorkerBuildMenu" });
  assert(explicitHudIntent.commandCardMode === "workerBuild", "HUD dispatch opens build menu through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginPlacement", building: KIND.DEPOT });
  assert(explicitHudIntent.placement?.building === KIND.DEPOT, "HUD dispatch starts placement through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginCommandTarget", target: "attack" });
  assert(explicitHudIntent.commandTarget === "attack", "HUD dispatch arms command targets through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginCommandTarget", target: "move" }, { shiftKey: true });
  assert(explicitHudIntent.commandComposer.shiftPreserved === true, "HUD dispatch preserves Shift for command targeting");

  const explicitInputIntent = new ClientIntent();
  explicitInputIntent.beginPlacement(KIND.BARRACKS);
  const facadeInput = Object.create(Input.prototype);
  let facadeSelectionCleared = 0;
  facadeInput.state = {
    endPlacement() {
      throw new Error("Input must use injected ClientIntent for placement cancellation");
    },
    clearSelection() {
      facadeSelectionCleared += 1;
    },
  };
  facadeInput.clientIntent = explicitInputIntent;
  facadeInput._cancel();
  assert(explicitInputIntent.placement === null, "Input cancellation clears placement through injected ClientIntent");
  assert(facadeSelectionCleared === 0, "Input placement cancellation does not fall through to selection clear");

  const labIntent = new ClientIntent();
  labIntent.beginLabTool({ kind: "fieldPoint" });
  const labCancelInput = Object.create(Input.prototype);
  let labSelectionCleared = 0;
  let labCancelReason = null;
  labCancelInput.state = { clearSelection() { labSelectionCleared += 1; } };
  labCancelInput.clientIntent = labIntent;
  labCancelInput.labToolController = {
    cancel(reason) {
      labCancelReason = reason;
      return labIntent.cancelLabTool(reason);
    },
  };
  labCancelInput._cancel();
  assert(labIntent.activeLabTool === null, "Input cancellation clears active lab tools through ClientIntent");
  assert(labCancelReason === "escape", "Input cancellation reports active lab-tool cancellation through the controller");
  assert(labSelectionCleared === 0, "Input lab tool cancellation does not fall through to selection clear");
}
