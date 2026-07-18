import { STATS } from "./config.js";

export function buildControlGroupSummaries(state, selected = [], controlPolicy = null) {
  const selectedIds = new Set((selected || []).map((entity) => entity.id));
  const selectedCount = selectedIds.size;
  const out = [];
  const groups = state?.controlGroups || [];
  for (let slot = 0; slot < groups.length; slot++) {
    const entities = typeof state?.controlGroupEntities === "function"
      ? state.controlGroupEntities(slot, { controlPolicy })
      : [];
    if (!entities || entities.length === 0) {
      out.push(null);
      continue;
    }

    const dominant = dominantControlGroupKind(entities);
    const st = STATS[dominant.kind] || {};
    out.push({
      key: slot === 9 ? "0" : String(slot + 1),
      count: entities.length,
      icon: st.icon || dominant.kind,
      label: st.label || dominant.kind,
      selected: controlGroupMatchesSelection(entities, selectedIds, selectedCount),
    });
  }
  return out;
}

export function controlGroupTabsSignature(groups) {
  return (groups || []).map((group) =>
    group ? `${group.key}:${group.count}:${group.icon}:${group.selected ? 1 : 0}` : "-",
  ).join("|");
}

export function renderControlGroupTabs(tabs, groups) {
  if (!tabs) return;
  const any = (groups || []).some(Boolean);
  tabs.classList.toggle("empty", !any);

  const frag = document.createDocumentFragment();
  for (const group of groups || []) {
    const slot = document.createElement("div");
    slot.className = "control-group-slot";
    if (group) {
      const tab = document.createElement("div");
      tab.className = "control-group-tab" + (group.selected ? " selected" : "");
      tab.setAttribute(
        "aria-label",
        `Control group ${group.key}: ${group.count} ${group.label}`,
      );
      tab.innerHTML =
        `<span class="control-group-key">${group.key}</span>` +
        `<span class="control-group-kind">${group.icon}</span>` +
        `<span class="control-group-count">${group.count}</span>`;
      slot.appendChild(tab);
    }
    frag.appendChild(slot);
  }

  tabs.innerHTML = "";
  tabs.appendChild(frag);
}

export function dominantControlGroupKind(entities) {
  const counts = new Map();
  let best = { kind: entities[0].kind, count: 0, first: 0 };
  for (let i = 0; i < entities.length; i++) {
    const kind = entities[i].kind;
    const entry = counts.get(kind) || { kind, count: 0, first: i };
    entry.count += 1;
    counts.set(kind, entry);
    if (
      entry.count > best.count ||
      (entry.count === best.count && entry.first < best.first)
    ) {
      best = entry;
    }
  }
  return best;
}

export function controlGroupMatchesSelection(entities, selectedIds, selectedCount) {
  if (selectedCount === 0 || entities.length !== selectedCount) return false;
  for (const entity of entities) {
    if (!selectedIds.has(entity.id)) return false;
  }
  return true;
}
