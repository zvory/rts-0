const BIRTHDAY_TIME_ZONE = "America/New_York";
const NEW_YORK_DATE = new Intl.DateTimeFormat("en-US", {
  timeZone: BIRTHDAY_TIME_ZONE,
  month: "numeric",
  day: "numeric",
});

export function isSoupmanBirthday(now = Date.now()) {
  const parts = NEW_YORK_DATE.formatToParts(new Date(now));
  const month = Number(parts.find((part) => part.type === "month")?.value);
  const day = Number(parts.find((part) => part.type === "day")?.value);
  return month === 8 && day === 11;
}

export class BirthdayBanner {
  constructor(root, {
    tankIconElements = [],
    mountTankIcon = null,
    now = () => Date.now(),
    setTimeoutFn = globalThis.setTimeout?.bind(globalThis),
    clearTimeoutFn = globalThis.clearTimeout?.bind(globalThis),
    createVisibilityObserver = typeof globalThis.MutationObserver === "function"
      ? (callback) => new globalThis.MutationObserver(callback)
      : null,
  } = {}) {
    this.root = root;
    this.tankIconElements = tankIconElements;
    this.mountTankIcon = mountTankIcon;
    this.now = now;
    this.setTimeoutFn = setTimeoutFn;
    this.clearTimeoutFn = clearTimeoutFn;
    this.iconControllers = [];
    this.refreshTimer = null;
    this.isBirthday = false;
    this.lobbyScreen = this.root?.closest?.(".screen") || null;
    this.visibilityObserver = null;

    if (!this.root) return;
    if (this.lobbyScreen && typeof createVisibilityObserver === "function") {
      this.visibilityObserver = createVisibilityObserver(() => this._syncIcons());
      this.visibilityObserver?.observe?.(this.lobbyScreen, {
        attributes: true,
        attributeFilter: ["hidden"],
      });
    }
    this._refresh();
  }

  destroy() {
    if (this.refreshTimer !== null) this.clearTimeoutFn?.(this.refreshTimer);
    this.refreshTimer = null;
    this.visibilityObserver?.disconnect?.();
    this.visibilityObserver = null;
    this._destroyIcons();
  }

  _refresh() {
    const timestamp = Number(new Date(this.now()));
    this.isBirthday = isSoupmanBirthday(timestamp);
    this.root.hidden = !this.isBirthday;
    this._syncIcons();

    const nextMinuteDelay = 60_050 - (timestamp % 60_000);
    this.refreshTimer = this.setTimeoutFn?.(() => this._refresh(), nextMinuteDelay) ?? null;
  }

  _syncIcons() {
    const active = this.isBirthday && !this.lobbyScreen?.hidden;
    if (active && this.iconControllers.length === 0 && typeof this.mountTankIcon === "function") {
      this.iconControllers = this.tankIconElements.map((element, index) => this.mountTankIcon(element, index));
    } else if (!active) {
      this._destroyIcons();
    }
  }

  _destroyIcons() {
    for (const controller of this.iconControllers) controller?.destroy?.();
    this.iconControllers = [];
  }
}
