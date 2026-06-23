// tests/client_contracts/command_composer_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { ABILITY } from "../../client/src/protocol.js";
import { CommandComposer } from "../../client/src/command_composer.js";

// ---------------------------------------------------------------------------
// Command composer
// ---------------------------------------------------------------------------
{
  const composer = new CommandComposer();
  let armed = composer.arm("attack", { now: 100 });
  assert(!armed.quickCast, "first command tap arms without quick-casting");
  armed = composer.arm("attack", { now: 220 });
  assert(armed.quickCast, "second same command tap inside the window requests quick-cast");

  let issued = composer.issue({ shiftKey: true });
  assert(issued.queued === true && issued.keepArmed === true, "Shift-click queues and preserves a tapped command");
  issued = composer.issue({ shiftKey: true });
  assert(issued.keepArmed === true, "Shift-preserved command can issue repeatedly");
  composer.releaseShift();
  assert(composer.target === null, "releasing Shift clears a Shift-preserved tapped command");

  composer.arm({ kind: "ability", ability: ABILITY.SMOKE }, { source: "hold", key: "KeyQ" });
  issued = composer.issue({ shiftKey: false });
  assert(
    issued.target.kind === "ability" &&
      issued.target.ability === ABILITY.SMOKE &&
      issued.keepArmed === true,
    "held ability key keeps the target armed after a click",
  );
  composer.releaseKey("KeyQ", { shiftKey: true });
  assert(composer.target?.ability === ABILITY.SMOKE, "Shift preserves the last held ability after key release");
  composer.releaseShift();
  assert(composer.target === null, "Shift release clears the preserved held ability");

  const tapPreservedComposer = new CommandComposer();
  const smokeTarget = { kind: "ability", ability: ABILITY.SMOKE };
  tapPreservedComposer.arm(smokeTarget, { now: 400 });
  tapPreservedComposer.hold(smokeTarget, "KeyD", { preserveTapOnRelease: true });
  tapPreservedComposer.releaseKey("KeyD", { shiftKey: false });
  assert(
    tapPreservedComposer.target?.ability === ABILITY.SMOKE,
    "tap-preserved ability key release keeps targeting armed before the click",
  );
  issued = tapPreservedComposer.issue({ shiftKey: false });
  assert(
    issued.target?.ability === ABILITY.SMOKE &&
      issued.keepArmed === false &&
      tapPreservedComposer.target === null,
    "tap-preserved ability click consumes targeting after the key was released",
  );

  const heldRepeatComposer = new CommandComposer();
  heldRepeatComposer.arm(smokeTarget, { now: 800 });
  heldRepeatComposer.hold(smokeTarget, "KeyD", { preserveTapOnRelease: true });
  issued = heldRepeatComposer.issue({ shiftKey: false });
  assert(issued.keepArmed === true, "held tap-preserved ability stays armed after a click while the key is down");
  heldRepeatComposer.releaseKey("KeyD", { shiftKey: false });
  assert(heldRepeatComposer.target === null, "held tap-preserved ability clears on keyup after issuing");

  composer.arm("move");
  composer.cancel();
  assert(composer.target === null, "cancel clears the armed command");
}

// ---------------------------------------------------------------------------
