// Project an ordinary command card into passive inspection without command affordances.
export function buildInspectionCard(ctx, buildCard) {
  const owner = Number((ctx.selection || []).find((e) => Number(e.owner) > 0)?.owner);
  if (!owner) return { kind: "spectator", signature: "spectator", slots: [] };
  const inspection = { ...ctx, commandSurfaceEnabled: true, readOnly: true,
    commandOwner: owner, playerId: owner, commandTarget: null, commandCardMode: null };
  const result = buildCard(inspection);
  return { ...result, readOnly: true, signature: `inspect:${owner}|${result.signature}`,
    slots: result.slots.map((slot) => slot && ({ ...slot,
      enabled: false, unaffordable: false, intent: null, contextIntent: null,
      onUnavailableIntent: null, repeatable: false, hotkey: "", hotkeyCode: "",
      contextHotkeyModifiers: [],
    })) };
}

export function commandSurfaceEnabled(ctx) {
  if (typeof ctx?.commandSurfaceEnabled === "boolean") return ctx.commandSurfaceEnabled;
  return !ctx?.spectator;
}

export function researchStatus(ctx, upgrade, definition, reason) {
  return (ctx.upgrades || []).includes(upgrade)
    ? { label: `${definition.label} ✓`, title: "Researched" }
    : { label: definition.label, title: reason };
}
