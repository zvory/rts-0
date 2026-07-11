// tests/client_contracts/ability_hotkey_targeting_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { ABILITY } from "../../client/src/protocol.js";

{
  const originalDocument = globalThis.document;
  const input = Object.create(Input.prototype);
  const issuedTargets = [];

  input.mouse = { x: 420, y: 260 };
  input.pointerLocked = false;
  input.cameraNavigation = null;
  input._panDrag = null;
  input._drag = null;
  input._dragging = false;
  input._placementDrag = null;
  input.screenOverlay = { setMarquee() {}, clearMarquee() {} };
  input._handleControlGroupHotkey = () => false;
  input._issueTargetedCommand = (p, ev) => {
    issuedTargets.push({ issuedAt: p, queued: !!ev.shiftKey });
  };
  input._eventScreenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  input._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  input._trackMouse = () => {};
  input._commitClickSelection = () => {
    throw new Error("Smoke targeting clicks must not fall through to selection");
  };
  input._commitBoxSelection = () => {
    throw new Error("Smoke targeting clicks must not fall through to box selection");
  };
  input.state = {};

  try {
    globalThis.document = smokeCommandCard(input);

    input.clientIntent = new ClientIntent();
    input._handleKeyDown(keyEvent("KeyD"));
    input._handleKeyUp(keyEvent("KeyD"));
    assert(
      input.clientIntent.commandTarget?.ability === ABILITY.SMOKE,
      "Smoke hotkey tap should stay armed after keyup",
    );
    input._onLeftDown({ x: 430, y: 270 }, { shiftKey: false });
    assert(
      issuedTargets.some((entry) => entry.issuedAt?.x === 430 && entry.issuedAt?.y === 270 && entry.queued === false),
      "Smoke hotkey tap should issue on the later click",
    );
    assert(
      input.clientIntent.commandTarget === null,
      "unqueued Smoke click after key release should consume targeting",
    );

    input.clientIntent = new ClientIntent();
    issuedTargets.length = 0;
    input._handleKeyDown(keyEvent("KeyD"));
    input._onLeftDown({ x: 440, y: 280 }, { shiftKey: false });
    assert(
      input.clientIntent.commandTarget?.ability === ABILITY.SMOKE,
      "held Smoke hotkey should keep targeting armed after a click",
    );
    input._handleKeyUp(keyEvent("KeyD"));
    assert(
      input.clientIntent.commandTarget === null,
      "held Smoke hotkey should clear on keyup after issuing",
    );
  } finally {
    globalThis.document = originalDocument;
  }
}

function keyEvent(code, mods = {}) {
  return {
    code,
    altKey: !!mods.altKey,
    ctrlKey: !!mods.ctrlKey,
    metaKey: !!mods.metaKey,
    shiftKey: !!mods.shiftKey,
    repeat: !!mods.repeat,
    preventDefault() {
      this.prevented = true;
    },
    stopPropagation() {
      this.stopped = true;
    },
  };
}

function smokeCommandCard(input) {
  return {
    getElementById(id) {
      assert(id === "command-card", "ability hotkeys should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "ability hotkeys should query hotkey buttons");
          return [{
            dataset: { hotkey: "D" },
            disabled: false,
            click() {
              input.clientIntent.beginCommandTarget(
                { kind: "ability", ability: ABILITY.SMOKE },
                { now: 1000 },
              );
            },
          }];
        },
      };
    },
  };
}
