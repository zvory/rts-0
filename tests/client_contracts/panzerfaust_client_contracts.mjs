// tests/client_contracts/panzerfaust_client_contracts.mjs
// Client-side Panzerfaust visual/event contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { EVENT, KIND, STATE } from "../../client/src/protocol.js";
import { createLiveRigDefinitions } from "../../client/src/renderer/rigs/live_routing.js";
import { createRigRenderContext, sampleRigAnimation } from "../../client/src/renderer/rigs/animation.js";

const start = {
  playerId: 1,
  spectator: false,
  players: [{ id: 1, teamId: 1 }, { id: 2, teamId: 2 }],
  map: { w: 20, h: 20, tileSize: 32, terrain: [], resources: [] },
};

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 10,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{
      id: 31,
      owner: 1,
      kind: KIND.PANZERFAUST,
      x: 300,
      y: 340,
      hp: 45,
      maxHp: 45,
      state: STATE.ATTACK,
      weaponFacing: 0,
    }],
    events: [
      { e: EVENT.PANZERFAUST_LAUNCH, from: 31, fromX: 300, fromY: 340, toX: 352, toY: 340, delayTicks: 15 },
      { e: EVENT.PANZERFAUST_IMPACT, x: 352, y: 340 },
      { e: EVENT.PANZERFAUST_LAUNCH, from: 32, toX: 360, toY: 340, delayTicks: 15 },
    ],
  });

  assert(state.livePanzerfaustShots(performance.now()).length === 0, "Panzerfaust impact clears matching in-flight tracer");
  assert(state.livePanzerfaustImpacts(performance.now()).length === 1, "Panzerfaust impact event creates a live impact cue");
  assert(state.weaponRecoil(31, KIND.PANZERFAUST, performance.now()) > 0, "Panzerfaust launch starts loaded-weapon recoil");
  assert(state.visibleTiles.length === 0, "Panzerfaust visual events do not stamp or extend client fog visibility");

  state.addPanzerfaustShot({
    e: EVENT.PANZERFAUST_LAUNCH,
    from: 31,
    fromX: 300,
    fromY: 340,
    toX: 352,
    toY: 340,
    delayTicks: 15,
  }, performance.now());
  assert(state.livePanzerfaustShots(performance.now()).length === 1, "Panzerfaust launch event creates a live tracer when impact has not arrived");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 20,
    steel: 0,
    oil: 0,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [{ id: 41, owner: 1, kind: KIND.PANZERFAUST, x: 96, y: 96, hp: 30, maxHp: 45, state: STATE.IDLE }],
    events: [],
  });
  state.setSelection([41]);
  state.setControlGroup(0, state.selection);
  state.applySnapshot({
    tick: 21,
    steel: 0,
    oil: 0,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [{ id: 41, owner: 1, kind: KIND.PANZERFAUST, x: 96, y: 96, hp: 30, maxHp: 45, state: STATE.IDLE, panzerfaustLoaded: false }],
    events: [],
  });

  assert(state.selection.has(41), "Panzerfaust reload preserves client selection");
  assert(state.selectedEntities()[0]?.kind === KIND.PANZERFAUST, "reloading Panzerfaust keeps its unit kind");
  assert(state.selectedEntities()[0]?.panzerfaustLoaded === false, "reloading Panzerfaust keeps projectile-loaded state");
  assert(state.controlGroups[0].join(",") === "41", "Panzerfaust reload preserves local control groups");
  assert(state.controlGroupEntities(0)[0]?.kind === KIND.PANZERFAUST, "control-group recall resolves the reloading Panzerfaust in place");
}

{
  const definitions = createLiveRigDefinitions();
  const definition = definitions.get(KIND.PANZERFAUST);
  const loadedEntity = {
    id: 51,
    owner: 1,
    kind: KIND.PANZERFAUST,
    x: 100,
    y: 100,
    hp: 45,
    maxHp: 45,
    state: STATE.IDLE,
    panzerfaustLoaded: true,
  };
  const unloadedEntity = { ...loadedEntity, panzerfaustLoaded: false };
  const loaded = sampleRigAnimation(definition, loadedEntity, createRigRenderContext(loadedEntity));
  const unloaded = sampleRigAnimation(definition, unloadedEntity, createRigRenderContext(unloadedEntity));

  assert(loaded.parts["part.pzf.warhead"].visible === true, "loaded Panzerfaust rig shows the warhead");
  assert(unloaded.parts["part.pzf.warhead"].visible === false, "reloading Panzerfaust rig hides the warhead");
}
