import {
  _beginFormationGesture,
  _finishFormationGesture,
  _updateFormationGesture,
} from "../../client/src/input/formation_gesture.js";
import { buildFormationLinePreview } from "../../client/src/input/formation_line.js";
import { drawFormationMovePreview } from "../../client/src/renderer/formation_line_preview.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

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
    _selectedOwnLandUnitIds: () => [1, 2],
    _intent: () => intent,
    _onRightClick: (point) => rightClicks.push(point),
    commandInteraction: { issueCommand: (command) => commands.push(command) },
  };
  return { input, commands, rightClicks, previews };
}

function attackHarness() {
  const h = harness();
  const targetedClicks = [];
  const feedback = [];
  let targetIssues = 0;
  let ended = 0;
  h.input._intent = () => ({
    commandTarget: "attack",
    updateFormationMovePreview(preview) {
      h.previews.push(preview);
    },
    clearFormationMovePreview() {},
    issueCommandTarget() {
      targetIssues += 1;
      return { keepArmed: false };
    },
    endCommandTarget() {
      ended += 1;
    },
  });
  h.input._issueTargetedCommand = (point) => {
    targetedClicks.push(point);
    return true;
  };
  h.input._addCommandFeedback = (kind, x, y, queued) => feedback.push({ kind, x, y, queued });
  return {
    ...h,
    targetedClicks,
    feedback,
    targetIssues: () => targetIssues,
    ended: () => ended,
  };
}

{
  const preview = buildFormationLinePreview(
    [{ x: 100, y: 100 }, { x: 300, y: 100 }],
    [
      { id: 1, kind: "rifleman", x: 290, y: 100 },
      { id: 2, kind: "rifleman", x: 110, y: 100 },
    ],
  );
  assert(preview.slots.find((slot) => slot.unitId === 1)?.x === 300,
    "preview assigns the rightmost unit to the rightmost slot like the server");
  assert(preview.slots.find((slot) => slot.unitId === 2)?.x === 100,
    "preview assigns the leftmost unit to the leftmost slot like the server");
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
  const h = attackHarness();
  _beginFormationGesture.call(h.input, { x: 100, y: 100 }, {}, "attackMove");
  _updateFormationGesture.call(h.input, { x: 195, y: 100 });

  const preview = h.previews.at(-1);
  assert(preview.kind === "attackMove", "attack-targeted left drag marks the shared preview as attack-move");
  assert(preview.slots.length === 0, "attack-move slots use the shared three-tile promotion threshold");

  _finishFormationGesture.call(h.input, { x: 195, y: 100 });
  assert(h.commands.length === 0, "a short attack-targeted drag does not issue formation orders");
  assert(h.targetedClicks.length === 1, "a short attack-targeted drag resolves as the existing targeted click");
  assert(h.targetIssues() === 1 && h.ended() === 1, "short attack targeting preserves command-composer lifetime");
}

{
  const h = attackHarness();
  _beginFormationGesture.call(h.input, { x: 100, y: 100 }, { shiftKey: true }, "attackMove");
  _updateFormationGesture.call(h.input, { x: 196, y: 100 }, { shiftKey: true });
  _finishFormationGesture.call(h.input, { x: 196, y: 100 }, { shiftKey: true });

  assert(h.targetedClicks.length === 0, "a promoted attack-move line does not issue the fallback targeted click");
  assert(h.commands.length === 2, "a promoted attack-move line issues one command per selected unit");
  assert(h.commands.every((command) => command.c === "attackMove" && command.units.length === 1),
    "each assigned slot receives an ordinary single-unit attack-move order");
  assert(new Set(h.commands.map((command) => `${command.x},${command.y}`)).size === 2,
    "selected units receive distinct points along the attack-move line");
  assert(h.commands.every((command) => command.queued === true), "Shift queues every attack-move slot order");
  assert(h.targetIssues() === 1 && h.ended() === 1, "attack-move line release consumes targeting once");
  assert(h.feedback.length === 1 && h.feedback[0].kind === "attack", "attack-move line release uses red command feedback");
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
  const graphics = new RecordingGraphics();
  drawFormationMovePreview(graphics, {
    points: [{ x: 0, y: 0 }, { x: 96, y: 0 }],
    slots: [],
  });
  const lineStyles = graphics.calls.filter(([name]) => name === "lineStyle");
  const circles = graphics.calls.filter(([name]) => name === "drawCircle");
  assert(lineStyles.some(([, width, color, alpha]) => width === 7 && color === 0x071018 && alpha === 0.72), "preview has a contrasting outer stroke");
  assert(lineStyles.some(([, width, , alpha]) => width === 3 && alpha === 1), "preview has an opaque colored inner stroke");
  assert(circles.length === 2, "preview marks both stroke endpoints");
}

{
  const graphics = new RecordingGraphics();
  drawFormationMovePreview(graphics, {
    kind: "attackMove",
    points: [{ x: 0, y: 0 }, { x: 96, y: 0 }],
    slots: [{ unitId: 1, x: 48, y: 0, radius: 10 }],
  });
  const redStyles = graphics.calls.filter(
    ([name, , color]) => name === "lineStyle" && color === 0xd47a5f,
  );
  assert(redStyles.length >= 2, "attack-move polyline and placement ring use the red enemy-selection color");
}

console.log("formation_gesture_contracts: ok");
