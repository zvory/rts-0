import { createCommandButton } from "./hud_command_dom.js";
import { resourceIconHtml } from "./resource_icons.js";

const DEFAULT_RESERVATIONS = Object.freeze({ steel: 200, oil: 100 });
const RESERVATION_STEP = 50;
const RESERVATION_MAX = 9_950;
const POINTER_HOLD_DELAY_MS = 200;

/**
 * Browser-local prototype for the hold-Tab in-game menu.
 *
 * The state deliberately does not enter GameState or the wire protocol. Interact may emulate the
 * held key through the narrow hold/release actions so the panel can be captured deterministically.
 */
export class TabMenu {
  constructor({
    root,
    button,
    settings = null,
    hotkeyProfiles = null,
    enabled = () => true,
    windowLike = globalThis.window,
  } = {}) {
    this.root = root || null;
    this.button = button || null;
    this.settings = settings;
    this.hotkeyProfiles = hotkeyProfiles;
    this.enabled = enabled;
    this.windowLike = windowLike;
    this.paused = false;
    this.reservations = { ...DEFAULT_RESERVATIONS };
    this.heldBy = new Set();
    this.destroyed = false;
    this.pointerHoldTimer = null;
    this.pointerId = null;
    this.pointerHoldOpened = false;
    this.suppressNextButtonClick = false;

    this.panel = document.createElement("aside");
    this.panel.id = "tab-menu";
    this.panel.className = "hud-panel tab-menu";
    this.panel.setAttribute("aria-labelledby", "tab-menu-title");
    this.panel.hidden = true;
    this.root?.appendChild(this.panel);

    this.originalButton = {
      ariaLabel: this.button?.getAttribute("aria-label"),
      title: this.button?.getAttribute("title"),
      ariaControls: this.button?.getAttribute("aria-controls"),
      ariaExpanded: this.button?.getAttribute("aria-expanded"),
      html: this.button?.innerHTML || "",
    };
    this.button?.parentElement?.classList.add("tab-menu-mode");
    if (this.button) {
      this.button.setAttribute("aria-label", "Settings; hold for in-game menu");
      this.button.setAttribute("title", "Click for Settings; hold Tab or press and hold for in-game menu");
      this.button.setAttribute("aria-controls", "settings-menu tab-menu");
      this.button.innerHTML =
        '<span class="hamburger-icon" aria-hidden="true"><i></i><i></i><i></i></span>';
    }

    this.onKeyDown = (event) => this.handleKeyDown(event);
    this.onKeyUp = (event) => this.handleKeyUp(event);
    this.onBlur = () => {
      this.release("keyboard");
      this.cancelPointerHold();
    };
    this.onPointerDown = (event) => {
      if (event.button !== 0) return;
      this.beginPointerHold(event);
    };
    this.onPointerUp = (event) => this.finishPointerHold(event);
    this.onPointerCancel = (event) => this.cancelPointerHold(event);
    this.onButtonClickCapture = (event) => {
      if (!this.suppressNextButtonClick) return;
      this.suppressNextButtonClick = false;
      event.preventDefault();
      event.stopImmediatePropagation();
    };

    this.windowLike?.addEventListener("keydown", this.onKeyDown, true);
    this.windowLike?.addEventListener("keyup", this.onKeyUp, true);
    this.windowLike?.addEventListener("blur", this.onBlur);
    this.windowLike?.addEventListener("pointerup", this.onPointerUp, true);
    this.windowLike?.addEventListener("pointercancel", this.onPointerCancel, true);
    this.button?.addEventListener("pointerdown", this.onPointerDown);
    this.button?.addEventListener("click", this.onButtonClickCapture, true);
    this.render();
  }

  status() {
    const profile = this.hotkeyProfiles?.getActiveProfile?.();
    return {
      visible: this.isOpen(),
      paused: this.paused,
      status: this.paused ? "Paused" : "Working",
      reservations: { ...this.reservations },
      pauseHotkey: profile?.mode === "direct" ? "A" : "Q",
      profileMode: profile?.mode === "direct" ? "classic" : "grid",
    };
  }

  isOpen() {
    return !this.destroyed && this.enabled() && !this.panel.hidden;
  }

  hold(source) {
    if (this.destroyed || !this.enabled()) return this.status();
    this.heldBy.add(source);
    this.settings?.close?.();
    this.render();
    this.panel.hidden = false;
    this.button?.setAttribute("aria-expanded", "true");
    this.button?.classList.add("active");
    return this.status();
  }

  release(source) {
    this.heldBy.delete(source);
    if (this.heldBy.size > 0) return this.status();
    this.panel.hidden = true;
    this.button?.setAttribute("aria-expanded", "false");
    this.button?.classList.remove("active");
    return this.status();
  }

  beginPointerHold(event) {
    if (this.destroyed || this.pointerHoldTimer != null || this.pointerId != null) return;
    this.pointerId = event.pointerId ?? 0;
    this.pointerHoldOpened = false;
    this.suppressNextButtonClick = false;
    this.pointerHoldTimer = this.windowLike?.setTimeout?.(() => {
      this.pointerHoldTimer = null;
      this.pointerHoldOpened = true;
      this.hold("pointer");
    }, POINTER_HOLD_DELAY_MS);
  }

  finishPointerHold(event) {
    if (this.pointerId == null || (event?.pointerId ?? 0) !== this.pointerId) return;
    this.clearPointerHoldTimer();
    if (this.pointerHoldOpened) {
      this.release("pointer");
      this.suppressNextButtonClick =
        event?.target === this.button || this.button?.contains?.(event?.target) === true;
    }
    this.pointerId = null;
    this.pointerHoldOpened = false;
  }

  cancelPointerHold(event = null) {
    if (event && this.pointerId != null && (event.pointerId ?? 0) !== this.pointerId) return;
    this.clearPointerHoldTimer();
    this.release("pointer");
    this.pointerId = null;
    this.pointerHoldOpened = false;
    this.suppressNextButtonClick = false;
  }

  clearPointerHoldTimer() {
    if (this.pointerHoldTimer == null) return;
    this.windowLike?.clearTimeout?.(this.pointerHoldTimer);
    this.pointerHoldTimer = null;
  }

  togglePause() {
    this.paused = !this.paused;
    this.render();
    return this.status();
  }

  adjustReservation(resource, delta) {
    if (resource !== "steel" && resource !== "oil") return this.status();
    const direction = delta < 0 ? -1 : 1;
    this.reservations[resource] = Math.max(
      0,
      Math.min(RESERVATION_MAX, this.reservations[resource] + direction * RESERVATION_STEP),
    );
    this.render();
    return this.status();
  }

  interact(input = {}) {
    switch (input.action) {
      case "hold": return this.hold("interact");
      case "release": return this.release("interact");
      case "toggle-pause":
        this.hold("interact");
        return this.togglePause();
      case "adjust":
        this.hold("interact");
        return this.adjustReservation(input.resource, input.delta);
      default:
        throw Object.assign(new Error("tab-menu action must be hold, release, toggle-pause, or adjust."), {
          code: "invalidInput",
        });
    }
  }

  handleKeyDown(event) {
    if (!this.enabled()) return;
    if (event.code === "Tab" && !isTextEntry(event.target)) {
      consume(event);
      if (!event.repeat) this.hold("keyboard");
      return;
    }
    if (!this.isOpen() || event.metaKey || event.ctrlKey || event.altKey || isTextEntry(event.target)) {
      return;
    }
    const profile = this.hotkeyProfiles?.getActiveProfile?.();
    const pauseCode = profile?.mode === "direct" ? "KeyA" : "KeyQ";
    if (event.code === pauseCode && !event.repeat) {
      consume(event);
      this.togglePause();
      return;
    }
    const action = {
      Digit1: ["steel", -1],
      Digit2: ["steel", 1],
      Digit3: ["oil", 1],
      Digit4: ["oil", -1],
    }[event.code];
    if (!action) return;
    consume(event);
    this.adjustReservation(action[0], action[1]);
  }

  handleKeyUp(event) {
    if (event.code !== "Tab") return;
    this.release("keyboard");
    if (!this.enabled()) return;
    consume(event);
  }

  render() {
    const state = this.status();
    const title = document.createElement("div");
    title.className = "tab-menu-header";
    title.innerHTML =
      '<h2 id="tab-menu-title">Auto-Build Settings</h2>' +
      '<span class="tab-menu-held-hint">HOLD <kbd>TAB</kbd></span>';

    const intro = document.createElement("p");
    intro.className = "tab-menu-intro";
    intro.textContent = "Set the resource floor that automatic production must preserve.";

    const pause = createCommandButton({
      icon: this.paused ? "▶" : "Ⅱ",
      label: this.paused ? "Paused" : "Working",
      hotkey: state.pauseHotkey,
      enabled: true,
      cls: `tab-menu-pause${this.paused ? " is-paused" : ""}`,
      title: this.paused ? "Resume all Auto-Build" : "Pause all Auto-Build",
      onClick: () => this.togglePause(),
    });
    pause.id = "tab-menu-autobuild-pause";
    pause.setAttribute("aria-pressed", String(this.paused));
    const pauseCopy = document.createElement("span");
    pauseCopy.className = "tab-menu-pause-copy";
    pauseCopy.innerHTML =
      `<strong>${this.paused ? "Auto-Build Paused" : "Auto-Build Working"}</strong>` +
      `<span>${this.paused ? "Automatic queues are stopped." : "Automatic queues are running."}</span>`;
    const pauseRow = document.createElement("div");
    pauseRow.className = "tab-menu-pause-row";
    pauseRow.append(pause, pauseCopy);

    const reservations = document.createElement("div");
    reservations.className = "tab-menu-reservations";
    reservations.append(
      this.reservationRow("steel", "Minimum Steel Reserve", "1", "2"),
      // The requested prototype binds Oil increase to 3 and decrease to 4, so the visual minus
      // and plus controls intentionally carry the reversed numeric order.
      this.reservationRow("oil", "Minimum Oil Reserve", "4", "3"),
    );

    this.panel.replaceChildren(title, intro, pauseRow, reservations);
  }

  reservationRow(resource, label, decreaseHotkey, increaseHotkey) {
    const section = document.createElement("section");
    section.className = "tab-menu-reservation";
    const heading = document.createElement("h3");
    heading.innerHTML = `${resourceIconHtml(resource)}<span>${label}</span>`;

    const controls = document.createElement("div");
    controls.className = "tab-menu-stepper";
    const decrease = createCommandButton({
      icon: "−",
      label: "Lower",
      hotkey: decreaseHotkey,
      enabled: true,
      cls: "tab-menu-step",
      title: `Reduce the ${resource} reservation`,
      onClick: () => this.adjustReservation(resource, -1),
    });
    const value = document.createElement("output");
    value.className = "tab-menu-reserve-value";
    value.id = `tab-menu-${resource}-reserve`;
    value.setAttribute("aria-label", `${label}: ${this.reservations[resource]}`);
    value.innerHTML =
      `${resourceIconHtml(resource)}` +
      `<strong>${this.reservations[resource].toLocaleString()}</strong>`;
    const increase = createCommandButton({
      icon: "+",
      label: "Raise",
      hotkey: increaseHotkey,
      enabled: true,
      cls: "tab-menu-step",
      title: `Increase the ${resource} reservation`,
      onClick: () => this.adjustReservation(resource, 1),
    });
    controls.append(decrease, value, increase);
    section.append(heading, controls);
    return section;
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.heldBy.clear();
    this.panel.hidden = true;
    this.clearPointerHoldTimer();
    this.pointerId = null;
    this.pointerHoldOpened = false;
    this.suppressNextButtonClick = false;
    this.windowLike?.removeEventListener("keydown", this.onKeyDown, true);
    this.windowLike?.removeEventListener("keyup", this.onKeyUp, true);
    this.windowLike?.removeEventListener("blur", this.onBlur);
    this.windowLike?.removeEventListener("pointerup", this.onPointerUp, true);
    this.windowLike?.removeEventListener("pointercancel", this.onPointerCancel, true);
    this.button?.removeEventListener("pointerdown", this.onPointerDown);
    this.button?.removeEventListener("click", this.onButtonClickCapture, true);
    this.button?.parentElement?.classList.remove("tab-menu-mode");
    if (this.button) {
      this.button.innerHTML = this.originalButton.html;
      restoreAttribute(this.button, "aria-label", this.originalButton.ariaLabel);
      restoreAttribute(this.button, "title", this.originalButton.title);
      restoreAttribute(this.button, "aria-controls", this.originalButton.ariaControls);
      restoreAttribute(this.button, "aria-expanded", this.originalButton.ariaExpanded);
      this.button.classList.remove("active");
    }
    this.panel.remove();
  }
}

function consume(event) {
  event.preventDefault();
  event.stopPropagation();
}

function isTextEntry(target) {
  return target instanceof HTMLElement &&
    (target.matches("input, textarea, select") || target.isContentEditable);
}

function restoreAttribute(element, name, value) {
  if (value == null) element.removeAttribute(name);
  else element.setAttribute(name, value);
}
