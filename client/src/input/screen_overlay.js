export class ScreenOverlay {
  constructor(drawMarquee) {
    if (typeof drawMarquee !== "function") throw new TypeError("ScreenOverlay requires drawMarquee");
    this._drawMarquee = drawMarquee;
    this._marquee = null;
  }

  setMarquee(rect) {
    this._marquee = rect ? Object.freeze({ ...rect }) : null;
    this._drawMarquee(this._marquee);
  }

  clearMarquee() {
    this.setMarquee(null);
  }

  snapshot() {
    return Object.freeze({
      version: 1,
      marquee: this._marquee ? Object.freeze({ ...this._marquee }) : null,
    });
  }

  destroy() {
    this.clearMarquee();
    this._drawMarquee = () => {};
  }
}
