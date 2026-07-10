// App-shell ownership for temporarily hiding DOM chrome around the normal Pixi viewport.
// It deliberately leaves world layers and the renderer untouched.

export const CLEAN_PRESENTATION_ATTRIBUTE = "data-clean-presentation";

export class CleanPresentation {
  constructor({ root = typeof document !== "undefined" ? document.getElementById("app") : null } = {}) {
    this.root = root;
    this.destroyed = false;
  }

  get active() {
    return !this.destroyed && this.root?.getAttribute?.(CLEAN_PRESENTATION_ATTRIBUTE) === "true";
  }

  set(enabled) {
    if (this.destroyed || !this.root?.setAttribute) return false;
    if (enabled) this.root.setAttribute(CLEAN_PRESENTATION_ATTRIBUTE, "true");
    else this.root.removeAttribute(CLEAN_PRESENTATION_ATTRIBUTE);
    return this.active;
  }

  destroy() {
    if (this.destroyed) return;
    this.set(false);
    this.destroyed = true;
  }
}
