import { cmd, KIND } from "../client/src/protocol.js";

export async function runPointInputContracts({ minimapHarness, lockedEvent, assert }) {

for (const queued of [false, true]) {
  const h = minimapHarness({ selected: [
    { id: 1, owner: 1, kind: KIND.TANK },
    { id: 2, owner: 1, kind: KIND.WORKER },
    { id: 3, owner: 2, kind: KIND.TANK },
  ], commandTarget: "pointTanks" });
  assert(h.router.pointerDown(lockedEvent(200, 300, 0, { shiftKey: queued })), "Point minimap click is consumed");
  assert(JSON.stringify(h.net.sent[0]) === JSON.stringify(cmd.pointTanks([1], 100, 100, queued)), "Point minimap filters tanks");
}

{
  const { _issueTargetedCommand } = await import("../client/src/input/commands.js");
  const selected = [{ id: 1, owner: 1, kind: KIND.TANK }, { id: 2, owner: 1, kind: KIND.WORKER }, { id: 3, owner: 2, kind: KIND.TANK }];
  const sent = [];
  const input = {
    clientIntent: { commandTarget: "pointTanks" },
    state: { playerId: 1, selectedEntities: () => selected, entities: new Map(selected.map((e) => [e.id, e])) },
    _selectedOwnUnitIds: () => [1, 2],
    _resourceAtScreen: () => null,
    _groundAtScreen: () => ({ x: 300, y: 400 }),
    commandInteraction: { issueCommand: (command) => sent.push(command) },
    _addCommandFeedback: () => {},
  };
  assert(_issueTargetedCommand.call(input, { x: 1, y: 2 }, { shiftKey: true }), "Point battlefield click handled");
  assert(JSON.stringify(sent[0]) === JSON.stringify(cmd.pointTanks([1], 300, 400, true)), "Point battlefield filters tanks and queues with Shift");
}
}
