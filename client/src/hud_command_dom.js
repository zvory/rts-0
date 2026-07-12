export function renderDescriptorCardDom(card, descriptorCard, buttonForDescriptor) {
  const frag = document.createDocumentFragment();
  const slots = Array.isArray(descriptorCard?.slots) ? descriptorCard.slots : [];
  for (let i = 0; i < 9; i++) {
    const descriptor = slots[i] || null;
    frag.appendChild(descriptor ? buttonForDescriptor(descriptor) : emptyCommandSlot());
  }
  card.innerHTML = "";
  card.appendChild(frag);
}

export function emptyCommandSlot() {
  const el = document.createElement("div");
  el.className = "cmd-empty";
  return el;
}

export function syncCooldownClockElement(button, cooldownClocks) {
  const clocks = Array.isArray(cooldownClocks) ? cooldownClocks : [];
  const arms = typeof button.querySelectorAll === "function"
    ? Array.from(button.querySelectorAll(".cmd-cd-arm"))
    : [];
  if (arms.length !== clocks.length) return;
  for (let i = 0; i < arms.length; i++) {
    arms[i].style.setProperty("--cooldown-rotation", `${clocks[i].rotationDeg.toFixed(1)}deg`);
  }
}

/**
 * Build a command-card button element.
 * @param {object} opts
 * @param {string} [opts.commandId] stable command identity for hotkey/profile tooling.
 * @param {number} [opts.slotIndex] rendered command-card grid slot.
 * @param {string} [opts.icon] glyph shown large.
 * @param {string} opts.label visible name.
 * @param {string} [opts.ability] ability id for dynamic cooldown-clock refreshes.
 * @param {string} [opts.hotkey] keyboard hint shown in a corner.
 * @param {{steel:number,oil:number}} [opts.cost] cost badge (omitted if absent).
 * @param {boolean} opts.enabled whether the action is currently available.
 * @param {boolean} [opts.unaffordable] true when tech is available but resources are short.
 * @param {string} [opts.title] tooltip / disabled reason.
 * @param {string} [opts.tooltipHtml] rich hover content rendered inside the button.
 * @param {string} [opts.cls] extra class (e.g. "cancel").
 * @param {string} [opts.countBadge] top-right ready count for partially-available abilities.
 * @param {{count:number,rotationDeg:number}[]} [opts.cooldownClocks] grouped cooldown clocks.
 * @param {boolean} [opts.repeatable] whether native keyboard repeat may trigger this button.
 * @param {boolean} [opts.autocastToggle] whether Alt+hotkey should invoke the autocast toggle.
 * @param {() => void} [opts.onMouseEnter] hover handler.
 * @param {() => void} [opts.onMouseLeave] hover-exit handler.
 * @param {() => void} [opts.onUnavailable] click handler for unaffordable buttons.
 * @param {(ev: MouseEvent) => void} [opts.onAltClick] Alt-click handler.
 * @param {(ev: MouseEvent) => void} [opts.onContextMenu] right-click handler.
 * @param {(ev: MouseEvent) => void} opts.onClick click handler (skipped when disabled).
 * @returns {HTMLButtonElement}
 */
export function createCommandButton(opts) {
  const btn = document.createElement("button");
  btn.type = "button";
  const classes = ["cmd-btn"];
  if (opts.cls) classes.push(opts.cls);
  if (opts.unaffordable) classes.push("unaffordable");
  btn.className = classes.join(" ");
  btn.disabled = !opts.enabled && !opts.unaffordable;
  if (opts.title) btn.title = opts.title;
  if (opts.hotkey) {
    btn.dataset.hotkey = opts.hotkey;
  }
  if (opts.commandId) btn.dataset.commandId = opts.commandId;
  if (Number.isInteger(opts.slotIndex)) btn.dataset.slotIndex = String(opts.slotIndex);
  if (opts.ability) btn.dataset.ability = opts.ability;
  if (opts.repeatable) btn.dataset.repeatable = "true";
  if (opts.autocastToggle) btn.dataset.autocastToggle = "true";
  if (typeof opts.onMouseEnter === "function") {
    btn.addEventListener("mouseenter", opts.onMouseEnter);
    btn.addEventListener("focus", opts.onMouseEnter);
  }
  if (typeof opts.onMouseLeave === "function") {
    btn.addEventListener("mouseleave", opts.onMouseLeave);
    btn.addEventListener("blur", opts.onMouseLeave);
  }
  if (typeof opts.onContextMenu === "function") {
    btn.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      opts.onContextMenu(ev);
    });
  }

  const costHtml = opts.cost
    ? `<span class="cmd-cost">` +
      (opts.cost.steel ? `<span class="c-steel">${opts.cost.steel}</span>` : "") +
      (opts.cost.steel && opts.cost.oil ? `<span class="c-sep">/</span>` : "") +
      (opts.cost.oil ? `<span class="c-oil">${opts.cost.oil}</span>` : "") +
      `</span>`
    : "";
  const cooldownClocks = Array.isArray(opts.cooldownClocks) ? opts.cooldownClocks : [];
  const cooldownHtml = cooldownClocks.length > 0
    ? `<span class="cmd-cooldowns" aria-hidden="true">` +
        `<span class="cmd-cd-clock">` +
          cooldownClocks.map((group) =>
            `<span class="cmd-cd-arm" style="--cooldown-rotation:${group.rotationDeg.toFixed(1)}deg"></span>`
          ).join("") +
        `</span>` +
      `</span>`
    : "";

  btn.innerHTML =
    `<span class="cmd-icon">${opts.icon || ""}</span>` +
    `<span class="cmd-label">${opts.label || ""}</span>` +
    (opts.hotkey ? `<span class="cmd-hotkey">${opts.hotkey}</span>` : "") +
    cooldownHtml +
    (opts.countBadge ? `<span class="cmd-ready-count">${opts.countBadge}</span>` : "") +
    costHtml +
    (opts.tooltipHtml ? `<span class="cmd-tooltip" role="tooltip">${opts.tooltipHtml}</span>` : "");

  if (
    typeof opts.onAltClick === "function" ||
    (opts.enabled && typeof opts.onClick === "function") ||
    (opts.unaffordable && typeof opts.onUnavailable === "function")
  ) {
    btn.addEventListener("click", (ev) => {
      ev.preventDefault();
      if (ev.altKey && typeof opts.onAltClick === "function") {
        opts.onAltClick(ev);
      } else if (opts.enabled && typeof opts.onClick === "function") {
        opts.onClick(ev);
      } else if (opts.unaffordable && typeof opts.onUnavailable === "function") {
        opts.onUnavailable(ev);
      }
    });
  }
  return btn;
}
