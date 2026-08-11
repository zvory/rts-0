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
  } = {}) {
    this.root = root;
    this.tankIconElements = tankIconElements;
    this.mountTankIcon = mountTankIcon;
    this.now = now;
    this.setTimeoutFn = setTimeoutFn;
    this.clearTimeoutFn = clearTimeoutFn;
    this.iconControllers = [];
    this.refreshTimer = null;

    if (!this.root) return;
    this._refresh();
  }

  destroy() {
    if (this.refreshTimer !== null) this.clearTimeoutFn?.(this.refreshTimer);
    this.refreshTimer = null;
    this._destroyIcons();
  }

  _refresh() {
    const timestamp = Number(new Date(this.now()));
    const visible = isSoupmanBirthday(timestamp);
    this.root.hidden = !visible;

    if (visible && this.iconControllers.length === 0 && typeof this.mountTankIcon === "function") {
      this.iconControllers = this.tankIconElements.map((element, index) => this.mountTankIcon(element, index));
    } else if (!visible) {
      this._destroyIcons();
    }

    const nextMinuteDelay = 60_050 - (timestamp % 60_000);
    this.refreshTimer = this.setTimeoutFn?.(() => this._refresh(), nextMinuteDelay) ?? null;
  }

  _destroyIcons() {
    for (const controller of this.iconControllers) controller?.destroy?.();
    this.iconControllers = [];
  }
}
