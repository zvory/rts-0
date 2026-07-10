// Focused pointer contracts for persistent Lab setup tools.

import { assert } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { KIND, STATE } from "../../client/src/protocol.js";

{
  const input = Object.create(Input.prototype);
  const events = [];
  const selections = [];
  let cancelled = null;
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginLabTool({
    kind: "spawnEntity",
    payload: { xField: "spawn-x" },
    keepArmedOnWorldClick: true,
  });
  input.labToolController = {
    consumeWorldClick(event) {
      events.push(event);
    },
    cancel(reason) {
      cancelled = reason;
      return input.clientIntent.cancelLabTool(reason);
    },
  };
  input.pointerLocked = false;
  input.cameraNavigation = null;
  input.renderer = { drawSelectionBox() {} };
  input._worldAt = (x, y) => ({ x: x + 100, y: y + 200 });
  input._commitClickSelection = (point) => selections.push(point);
  input._eventScreenPos = () => ({ x: 12, y: 24 });
  input._trackMouse = () => {};
  input._routeLockedPointerUp = () => false;
  input._finishTankTrapPlacementDrag = () => false;
  input._onLeftDown({ x: 12, y: 24 }, { shiftKey: true });
  assert(events.length === 0, "active Lab spawn tool waits for a completed click before placing");
  input._handleMouseUp({ button: 0, shiftKey: true });
  assert(events.length === 1, "active Lab spawn tool consumes the completed world click");
  assert(events[0].x === 112 && events[0].y === 224, "Lab tool click callback receives exact world coordinates");
  assert(events[0].tool.payload.xField === "spawn-x", "Lab tool click callback receives current tool payload");
  assert(input.clientIntent.activeLabTool !== null, "persistent Lab spawn tool stays armed after a world click");
  assert(cancelled === null, "persistent Lab spawn tool does not cancel on world click");
  input._onLeftDown({ x: 20, y: 28 }, {});
  input._eventScreenPos = () => ({ x: 20, y: 28 });
  input._handleMouseUp({ button: 0, shiftKey: false });
  assert(events.length === 2, "persistent Lab spawn tool places again on the next completed click");
  assert(input._drag == null && selections.length === 0, "Lab tool click does not fall through to selection drag");
}

{
  const input = Object.create(Input.prototype);
  const events = [];
  const selections = [];
  const boxes = [];
  let cancelled = null;
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginLabTool({
    kind: "spawnEntity",
    keepArmedOnWorldClick: true,
    paintOnDrag: true,
  });
  input.labToolController = {
    consumeWorldClick(event) {
      events.push(event);
    },
    cancel(reason) {
      cancelled = reason;
      return input.clientIntent.cancelLabTool(reason);
    },
  };
  input.pointerLocked = false;
  input.cameraNavigation = null;
  input.renderer = { drawSelectionBox(box) { boxes.push(box); } };
  input.state = { map: { width: 8, height: 8, tileSize: 32 } };
  let pointer = { x: 12, y: 24 };
  input._screenPos = () => pointer;
  input._eventScreenPos = () => pointer;
  input._worldAt = (x, y) => ({ x, y });
  input._trackMouse = () => {};
  input._routeLockedPointerMove = () => false;
  input._routeLockedPointerUp = () => false;
  input._finishTankTrapPlacementDrag = () => false;
  input._commitBoxSelection = (drag) => selections.push(drag);
  input._onLeftDown({ x: 12, y: 24 }, {});
  pointer = { x: 110, y: 42 };
  input._handleMouseMove({});
  input._handleMouseUp({ button: 0, shiftKey: false });
  assert(events.length === 5, "dragging with a Lab spawn tool paints each crossed map tile");
  assert(
    events.map((event) => `${event.x},${event.y}`).join("|") === "16,16|48,16|48,48|80,48|112,48",
    "Lab spawn painting emits deterministic tile-center positions along the drag stroke",
  );
  assert(cancelled === null, "drag painting does not cancel an active Lab spawn tool");
  assert(input.clientIntent.activeLabTool !== null, "drag painting keeps the active Lab spawn tool armed");
  assert(selections.length === 0, "drag painting does not fall through to box selection");
  assert(!boxes.some(Boolean), "drag painting does not draw a selection box");
}

{
  const input = Object.create(Input.prototype);
  const clickEvents = [];
  const boxEvents = [];
  const selections = [];
  const boxes = [];
  const removable = [
    { id: 61, owner: 2, kind: KIND.RIFLEMAN, x: 32, y: 32, hp: 45, maxHp: 45, state: STATE.IDLE },
    { id: 62, owner: 1, kind: KIND.WORKER, x: 72, y: 72, hp: 40, maxHp: 40, state: STATE.IDLE },
    { id: 63, owner: 1, kind: KIND.CITY_CENTRE, x: 64, y: 64, hp: 1000, maxHp: 1000, state: STATE.IDLE },
    { id: 64, owner: 2, kind: KIND.RIFLEMAN, x: 48, y: 48, hp: 45, maxHp: 45, state: STATE.IDLE, shotReveal: true },
  ];
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginLabTool({
    kind: "removeSelectableUnits",
    keepArmedOnWorldClick: true,
    consumeBoxSelection: true,
    keepArmedOnBoxSelection: true,
  });
  input.labToolController = {
    consumeWorldClick(event) {
      clickEvents.push(event);
    },
    consumeBoxSelection(event) {
      boxEvents.push(event);
    },
    cancel(reason) {
      return input.clientIntent.cancelLabTool(reason);
    },
  };
  input.state = {
    spectator: true,
    map: { width: 8, height: 8, tileSize: 32 },
    controlPolicy: {
      kind: "lab",
      canControlOwner: () => true,
      canSelectEntity: (entity) => !!entity && Number(entity.owner) > 0 && !entity.shotReveal && !entity.visionOnly,
    },
    entitiesInterpolated() {
      return removable;
    },
  };
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  input.pointerLocked = false;
  input.cameraNavigation = null;
  input.renderer = { drawSelectionBox(box) { boxes.push(box); } };
  input._trackMouse = () => {};
  input._routeLockedPointerMove = () => false;
  input._routeLockedPointerUp = () => false;
  input._finishTankTrapPlacementDrag = () => false;
  input._commitBoxSelection = (drag) => selections.push(drag);
  input._worldAt = Input.prototype._worldAt;
  input._entityAtWorld = Input.prototype._entityAtWorld;
  input._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  input._entityIntersectsRect = Input.prototype._entityIntersectsRect;
  input._closestIdsToPoint = Input.prototype._closestIdsToPoint;
  input._eventScreenPos = () => ({ x: 104, y: 64 });
  input._onLeftDown({ x: 104, y: 64 }, {});
  input._handleMouseUp({ button: 0, shiftKey: false });
  assert(
    clickEvents.length === 1 && clickEvents[0].entityIds.join(",") === "63" && clickEvents[0].entityId === 63,
    "Lab remove tool click receives the selectable building under the cursor",
  );
  assert(input.clientIntent.activeLabTool !== null, "Lab remove tool stays armed after click delete");
  let pointer = { x: 90, y: 90 };
  input._screenPos = () => pointer;
  input._eventScreenPos = () => pointer;
  input._onLeftDown({ x: 10, y: 10 }, {});
  input._handleMouseMove({});
  input._handleMouseUp({ button: 0, shiftKey: false });
  assert(boxEvents.length === 1, "Lab remove tool consumes box selections");
  assert(
    boxEvents[0].entityIds.join(",") === "61,63,62",
    "Lab remove tool box selection receives selectable units and buildings, excluding shot reveals",
  );
  assert(selections.length === 0, "Lab remove tool box selection does not fall through to normal selection");
  assert(input.clientIntent.activeLabTool !== null, "Lab remove tool stays armed after box delete");
  assert(boxes.some(Boolean), "Lab remove tool box selection still draws the selection box");
}

{
  const input = Object.create(Input.prototype);
  let cancelled = null;
  input.clientIntent = new ClientIntent();
  const tool = input.clientIntent.beginLabTool({ kind: "fieldPoint" });
  input.clientIntent.updateLabToolPreview({ toolId: tool.id, x: 5, y: 6 });
  input.labToolController = {
    cancel(reason) {
      cancelled = reason;
      return input.clientIntent.cancelLabTool(reason);
    },
  };
  input._selectedOwnUnitIds = () => {
    throw new Error("right-click Lab tool cancellation must not issue normal commands");
  };
  input._onRightClick({ x: 5, y: 6 }, {});
  assert(input.clientIntent.activeLabTool === null, "right-click cancels an active Lab tool");
  assert(input.clientIntent.labToolPreview === null, "right-click clears the active Lab-tool preview");
  assert(cancelled === "rightClick", "right-click Lab tool cancellation flows through the controller");
}
