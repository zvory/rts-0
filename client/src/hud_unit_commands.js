import { KIND } from "./protocol.js";

export function moveDescriptor(ctx, unitIds) {
  return {
    id: "unit:move",
    commandId: "unit.move",
    kind: "button",
    action: "move",
    intent: { type: "beginCommandTarget", target: "move" },
    icon: "MV",
    label: "Move",
    title: "Move to a target point",
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "move" ? "active" : "",
  };
}

export function attackDescriptor(ctx, unitIds) {
  return {
    id: "unit:attack",
    commandId: "unit.attack",
    kind: "button",
    action: "attack",
    intent: { type: "beginCommandTarget", target: "attack" },
    icon: "ATG",
    label: "Attack",
    title: "Attack a target or attack-move to a point",
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "attack" ? "active" : "",
  };
}

export function holdDescriptor(unitIds) {
  return {
    id: "unit:hold",
    commandId: "unit.holdPosition",
    kind: "button",
    action: "holdPosition",
    intent: { type: "holdPosition", unitIds },
    icon: "HLD",
    label: "Hold",
    title: "Hold position",
    enabled: unitIds.length > 0,
  };
}

export function stopDescriptor(unitIds) {
  return {
    id: "unit:stop",
    commandId: "unit.stop",
    kind: "button",
    action: "stop",
    intent: { type: "stop", unitIds },
    icon: "STP",
    label: "Stop",
    title: "Stop current orders",
    enabled: unitIds.length > 0,
  };
}

export function setupSupportWeaponDescriptor(ctx, setupWeapons) {
  const mortars = setupWeapons.filter((entity) => entity.kind === KIND.MORTAR_TEAM);
  const mortarOnly = mortars.length > 0 && mortars.length === setupWeapons.length;
  return {
    id: "unit:setup",
    commandId: "unit.setupSupportWeapon",
    kind: "button",
    action: "setupAntiTankGuns",
    intent: mortarOnly
      ? {
          type: "setupMortars",
          unitIds: mortars.map((entity) => entity.id),
          x: Number.isFinite(mortars[0]?.x) ? mortars[0].x : 0,
          y: Number.isFinite(mortars[0]?.y) ? mortars[0].y : 0,
        }
      : { type: "beginCommandTarget", target: "setupAntiTankGuns" },
    icon: "SET",
    label: "Set Up",
    title: mortarOnly
      ? "Set up selected mortars in place (hold Shift to queue)"
      : "Set up selected support weapons toward a target point",
    enabled: true,
    cls: ctx.commandTarget === "setupAntiTankGuns" ? "active" : "",
  };
}
