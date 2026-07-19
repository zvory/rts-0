import {
  _beginFormationGesture,
  _finishFormationGesture,
  _updateFormationGesture,
} from "../../client/src/input/formation_gesture.js";
import { drawFormationMovePreview } from "../../client/src/renderer/formation_line_preview.js";

function assert(condition, message) {
  if (!condition) throw new Error(message || "Assertion failed");
}

function harness() {
  const commands = [];
  const rightClicks = [];
  const previews = [];
  const intent = {
    updateFormationMovePreview(preview) {
      previews.push(preview);
    },
    clearFormationMovePreview() {},
  };
  const input = {
    state: {
      map: { tileSize: 32 },
      selectedEntities: () => [
        { id: 1, kind: "rifleman", x: 0, y: 0 },
        { id: 2, kind: "rifleman", x: 32, y: 0 },
      ],
    },
    _groundAtScreen: (x, y) => ({ x, y }),
    _selectedOwnUnitIds: () => [1, 2],
    _intent: () => intent,
    _onRightClick: (point) => rightClicks.push(point),
    commandInteraction: { issueCommand: (command) => commands.push(command) },
  };
  return { input, commands, rightClicks, previews };
}

{
  const h = harness();
  _beginFormationGesture.call(h.input, { x: 100, y: 100 });
  _updateFormationGesture.call(h.input, { x: 195, y: 100 });

  const preview = h.previews.at(-1);
  assert(preview.points.length >= 2, "a sub-threshold right drag visibly previews its stroke");
  assert(preview.slots.length === 0, "destination slots wait until the stroke reaches three tiles");

  _finishFormationGesture.call(h.input, { x: 195, y: 100 });
  assert(h.commands.length === 0, "a 95-world-pixel stroke does not issue a formation move");
  assert(h.rightClicks.length === 1, "a stroke shorter than three tiles resolves as a right-click");
}

{
  const h = harness();
  _beginFormationGesture.call(h.input, { x: 100, y: 100 });
  _updateFormationGesture.call(h.input, { x: 196, y: 100 });

  const preview = h.previews.at(-1);
  assert(preview.slots.length === 2, "destination slots appear once the stroke reaches three tiles");

  _finishFormationGesture.call(h.input, { x: 196, y: 100 });
  assert(h.rightClicks.length === 0, "a three-tile stroke does not fall back to right-click");
  assert(h.commands.length === 1 && h.commands[0].c === "formationMove", "a three-tile stroke issues a formation move");
}

{
  const calls = [];
  const graphics = new Proxy({}, {
    get(_target, property) {
      return (...args) => calls.push([property, ...args]);
    },
  });
  drawFormationMovePreview(graphics, {
    points: [{ x: 0, y: 0 }, { x: 96, y: 0 }],
    slots: [],
  });
  const lineStyles = calls.filter(([name]) => name === "lineStyle");
  const circles = calls.filter(([name]) => name === "drawCircle");
  assert(lineStyles.some(([, width, color, alpha]) => width === 7 && color === 0x071018 && alpha === 0.72), "preview has a contrasting outer stroke");
  assert(lineStyles.some(([, width, , alpha]) => width === 3 && alpha === 1), "preview has an opaque colored inner stroke");
  assert(circles.length === 2, "preview marks both stroke endpoints");
}

console.log("formation_gesture_contracts: ok");
