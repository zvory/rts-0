import { createCommandButton } from "./hud_command_dom.js";
import { cmd } from "./protocol.js";
import { resourceIconHtml } from "./resource_icons.js";

const DEFAULT_RESERVATIONS = Object.freeze({ steel: 0, oil: 0 });
const RESERVATION_STEP = 100;
const RESERVATION_MAX = 9_950;
const POINTER_HOLD_DELAY_MS = 200;

/**
 * Hold-Tab controls for the player's authoritative automatic-production settings.
 */
export class TabMenu {
  constructor({
    root,
    settings = null,
    hotkeyProfiles = null,
    state = null,
    commandInteraction = null,
    enabled = () => true,
    onOpenChange = null,
    windowLike = globalThis.window,
  } = {}) {
    this.root = root || null;
    this.settings = settings;
    this.hotkeyProfiles = hotkeyProfiles;
    this.state = state;
    this.commandInteraction = commandInteraction;
    this.enabled = enabled;
    this.onOpenChange = onOpenChange;
    this.windowLike = windowLike;
    const initial = authoritativeSettings(this.state);
    this.paused = initial.paused;
    this.reservations = { steel: initial.reserveSteel, oil: initial.reserveOil };
    this.pendingClientSeq = null;
    this.heldBy = new Set();
    this.destroyed = false;
    this.pointerHoldTimer = null;
    this.pointerId = null;
    this.pointerHoldOpened = false;
    this.suppressNextButtonClick = false;

    this.button = document.createElement("button");
    this.button.id = "tab-menu-button";
    this.button.type = "button";
    this.button.className = "tab-menu-button";
    this.button.setAttribute("aria-label", "Hold for Auto-Build menu");
    this.button.setAttribute("title", "Hold Tab or press and hold for Auto-Build menu");
    this.button.setAttribute("aria-controls", "tab-menu");
    this.button.setAttribute("aria-expanded", "false");
    this.button.innerHTML =
      '<span class="hamburger-icon" aria-hidden="true"><i></i><i></i><i></i></span>';

    this.panel = document.createElement("aside");
    this.panel.id = "tab-menu";
    this.panel.className = "hud-panel tab-menu";
    this.panel.setAttribute("aria-labelledby", "tab-menu-title");
    this.panel.hidden = true;
    this.root?.append(this.button, this.panel);

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
    this.syncFromState();
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
    const wasOpen = this.isOpen();
    this.heldBy.add(source);
    this.settings?.close?.();
    this.render();
    this.panel.hidden = false;
    this.button?.setAttribute("aria-expanded", "true");
    this.button?.classList.add("active");
    if (!wasOpen) this.onOpenChange?.(true);
    return this.status();
  }

  release(source) {
    const wasOpen = this.isOpen();
    this.heldBy.delete(source);
    if (this.heldBy.size > 0) return this.status();
    this.panel.hidden = true;
    this.button?.setAttribute("aria-expanded", "false");
    this.button?.classList.remove("active");
    if (wasOpen) this.onOpenChange?.(false);
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
    this.commitSettings();
    return this.status();
  }

  adjustReservation(resource, delta) {
    if (resource !== "steel" && resource !== "oil") return this.status();
    const direction = delta < 0 ? -1 : 1;
    this.reservations[resource] = Math.max(
      0,
      Math.min(RESERVATION_MAX, this.reservations[resource] + direction * RESERVATION_STEP),
    );
    this.commitSettings();
    return this.status();
  }

  commitSettings() {
    const issued = this.commandInteraction?.issueCommand?.(
      cmd.setAutoBuildSettings(
        this.paused,
        this.reservations.steel,
        this.reservations.oil,
      ),
      { predictMovement: false },
    );
    if (issued?.sent && Number.isInteger(issued.clientSeq)) {
      this.pendingClientSeq = Math.max(this.pendingClientSeq || 0, issued.clientSeq);
    } else if (this.commandInteraction && issued !== true) {
      this.syncFromState({ force: true });
    }
    this.render();
  }

  syncFromState({ force = false } = {}) {
    if (!this.state) return;
    const ack = Number(this.state.autoBuild?.ack);
    if (
      !force &&
      Number.isInteger(this.pendingClientSeq) &&
      (!Number.isInteger(ack) || ack < this.pendingClientSeq)
    ) {
      return;
    }
    if (Number.isInteger(this.pendingClientSeq) && Number.isInteger(ack) && ack >= this.pendingClientSeq) {
      this.pendingClientSeq = null;
    }
    const next = authoritativeSettings(this.state);
    this.paused = next.paused;
    this.reservations.steel = next.reserveSteel;
    this.reservations.oil = next.reserveOil;
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
      Digit3: ["oil", -1],
      Digit4: ["oil", 1],
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
      this.reservationRow("oil", "Minimum Oil Reserve", "3", "4"),
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
    this.pendingClientSeq = null;
    this.windowLike?.removeEventListener("keydown", this.onKeyDown, true);
    this.windowLike?.removeEventListener("keyup", this.onKeyUp, true);
    this.windowLike?.removeEventListener("blur", this.onBlur);
    this.windowLike?.removeEventListener("pointerup", this.onPointerUp, true);
    this.windowLike?.removeEventListener("pointercancel", this.onPointerCancel, true);
    this.button?.removeEventListener("pointerdown", this.onPointerDown);
    this.button?.removeEventListener("click", this.onButtonClickCapture, true);
    this.button.remove();
    this.panel.remove();
  }
}

function authoritativeSettings(state) {
  const settings = state?.autoBuild;
  return {
    paused: settings?.paused === true,
    reserveSteel: boundedReserve(settings?.reserveSteel, DEFAULT_RESERVATIONS.steel),
    reserveOil: boundedReserve(settings?.reserveOil, DEFAULT_RESERVATIONS.oil),
  };
}

function boundedReserve(value, fallback) {
  return Number.isInteger(value) && value >= 0 && value <= RESERVATION_MAX ? value : fallback;
}

function consume(event) {
  event.preventDefault();
  event.stopPropagation();
}

function isTextEntry(target) {
  return target instanceof HTMLElement &&
    (target.matches("input, textarea, select") || target.isContentEditable);
}
