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
