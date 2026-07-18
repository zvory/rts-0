import { PLAYER_PALETTE } from "./config.js";
import { resourceIconHtml } from "./resource_icons.js";

export function restoreSinglePlayerResourceShell(elHud) {
  if (!elHud) return { elSteel: null, elOil: null, elSupply: null };
  elHud.innerHTML =
    `<div class="res">${resourceIconHtml("steel")}<span id="res-steel">0</span></div>` +
    `<div class="res">${resourceIconHtml("oil")}<span id="res-oil">0</span></div>` +
    `<div class="res">${resourceIconHtml("supply")}<span id="res-supply">0 / 0</span></div>`;
  return {
    elSteel: elHud.querySelector("#res-steel"),
    elOil: elHud.querySelector("#res-oil"),
    elSupply: elHud.querySelector("#res-supply"),
  };
}

export function renderSinglePlayerResources({
  state,
  elHud,
  elSteel,
  elOil,
  elSupply,
  currentSig,
  recordDiagnostic = () => {},
}) {
  const resources = state?.resources || { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
  let steelEl = elSteel;
  let oilEl = elOil;
  let supplyEl = elSupply;
  let nextSig = currentSig;

  if ((nextSig && nextSig.startsWith("multi:")) || !steelEl || !oilEl || !supplyEl) {
    const restored = restoreSinglePlayerResourceShell(elHud);
    steelEl = restored.elSteel;
    oilEl = restored.elOil;
    supplyEl = restored.elSupply;
    nextSig = null;
  }

  const steel = resources.steel ?? 0;
  const oil = resources.oil ?? 0;
  const used = resources.supplyUsed ?? 0;
  const cap = resources.supplyCap ?? 0;
  const sig = `single:${steel}:${oil}:${used}:${cap}`;
  if (sig === nextSig) {
    recordDiagnostic("hud.dirty.resources.hit");
    return { sig: nextSig, elSteel: steelEl, elOil: oilEl, elSupply: supplyEl };
  }

  recordDiagnostic("hud.dirty.resources.miss");
  if (steelEl) steelEl.textContent = String(steel);
  if (oilEl) oilEl.textContent = String(oil);
  if (supplyEl) {
    supplyEl.textContent = `${used} / ${cap}`;
    supplyEl.classList.toggle("supply-capped", cap > 0 && used >= cap);
  }
  return { sig, elSteel: steelEl, elOil: oilEl, elSupply: supplyEl };
}

export function renderAllPlayersResources({
  state,
  playerResources,
  elHud,
  currentSig,
  recordDiagnostic = () => {},
}) {
  if (!elHud) return { sig: currentSig };

  const sig = "multi:" + playerResources.map(
    (p) => `${p.id}:${p.steel}:${p.oil}:${p.supplyUsed}:${p.supplyCap}:${p.apm}`,
  ).join("|");
  if (sig === currentSig) {
    recordDiagnostic("hud.dirty.resources.hit");
    return { sig: currentSig };
  }

  recordDiagnostic("hud.dirty.resources.miss");
  const players = state?.players || [];
  const frag = document.createDocumentFragment();
  for (const resources of playerResources) {
    const playerInfo = players.find((p) => p.id === resources.id);
    const idx = players.indexOf(playerInfo);
    const color = (playerInfo && playerInfo.color) || PLAYER_PALETTE[idx % PLAYER_PALETTE.length] || "#888";
    const name = (playerInfo && playerInfo.name) || `P${resources.id}`;
    const supplyCapped = resources.supplyCap > 0 && resources.supplyUsed >= resources.supplyCap;
    const apm = Math.max(0, Math.trunc(Number(resources.apm) || 0));

    const row = document.createElement("div");
    row.className = "res replay-player-res";
    row.innerHTML =
      `<span class="replay-player-dot" style="background:${color}" title="${name}"></span>` +
      `${resourceIconHtml("steel")}<span class="replay-res-val">${resources.steel}</span>` +
      `${resourceIconHtml("oil")}<span class="replay-res-val">${resources.oil}</span>` +
      `${resourceIconHtml("supply")}` +
      `<span class="replay-res-val${supplyCapped ? " supply-capped" : ""}">${resources.supplyUsed} / ${resources.supplyCap}</span>` +
      `<span class="replay-res-val replay-apm-val" title="Current actions per minute: ${apm}" aria-label="${apm} actions per minute">${apm}</span>`;
    frag.appendChild(row);
  }

  elHud.innerHTML = "";
  elHud.appendChild(frag);
  return { sig };
}
