export const ANALYTICS_CONSENT_STORAGE_KEY = "rts.analyticsConsent.v1";

export function analyticsMeasurementId(documentObj = globalThis.document) {
  const value = documentObj
    ?.querySelector?.('meta[name="rts-google-analytics-id"]')
    ?.getAttribute?.("content")
    ?.trim();
  return /^G-[A-Z0-9]{1,32}$/.test(value || "") ? value : "";
}

function browserStorage(windowObj) {
  try {
    return windowObj?.localStorage;
  } catch {
    return null;
  }
}

export class AnalyticsConsent {
  constructor({
    documentObj = globalThis.document,
    windowObj = globalThis.window,
    storage = browserStorage(windowObj),
    measurementId = analyticsMeasurementId(documentObj),
  } = {}) {
    this.document = documentObj;
    this.window = windowObj;
    this.storage = storage;
    this.measurementId = measurementId;
    this.banner = null;
    this.acceptButton = null;
    this.declineButton = null;
    this.loaded = false;
    this._accept = () => this._choose("granted");
    this._decline = () => this._choose("denied");
  }

  start() {
    if (!/^G-[A-Z0-9]{1,32}$/.test(this.measurementId || "")) return false;

    const preference = this._readPreference();
    if (preference === "granted") {
      this._loadGoogleTag();
    } else if (preference !== "denied") {
      this._showBanner();
    }
    return true;
  }

  destroy() {
    this.acceptButton?.removeEventListener?.("click", this._accept);
    this.declineButton?.removeEventListener?.("click", this._decline);
    this.banner?.remove?.();
    this.banner = null;
    this.acceptButton = null;
    this.declineButton = null;
  }

  _readPreference() {
    try {
      return this.storage?.getItem?.(ANALYTICS_CONSENT_STORAGE_KEY) || "";
    } catch {
      return "";
    }
  }

  _choose(preference) {
    try {
      this.storage?.setItem?.(ANALYTICS_CONSENT_STORAGE_KEY, preference);
    } catch {
      // Storage can be unavailable in private browsing; honor this page's choice anyway.
    }
    this.destroy();
    if (preference === "granted") this._loadGoogleTag();
  }

  _showBanner() {
    if (!this.document?.createElement || !this.document?.body?.appendChild || this.banner) return;

    const banner = this.document.createElement("aside");
    banner.className = "analytics-consent";
    banner.setAttribute("role", "dialog");
    banner.setAttribute("aria-label", "Analytics preferences");

    const copy = this.document.createElement("p");
    copy.textContent =
      "Allow Google Analytics? It helps us understand visits, approximate location, " +
      "traffic sources, and device types using a pseudonymous cookie.";

    const actions = this.document.createElement("div");
    actions.className = "analytics-consent-actions";

    const decline = this.document.createElement("button");
    decline.className = "btn";
    decline.type = "button";
    decline.textContent = "Decline";
    decline.addEventListener("click", this._decline);

    const accept = this.document.createElement("button");
    accept.className = "btn primary";
    accept.type = "button";
    accept.textContent = "Allow analytics";
    accept.addEventListener("click", this._accept);

    actions.append(decline, accept);
    banner.append(copy, actions);
    this.document.body.appendChild(banner);
    this.banner = banner;
    this.acceptButton = accept;
    this.declineButton = decline;
  }

  _loadGoogleTag() {
    if (this.loaded || !this.document?.createElement || !this.document?.head?.appendChild) return;
    this.loaded = true;

    const dataLayer = this.window.dataLayer || [];
    this.window.dataLayer = dataLayer;
    this.window.gtag = function gtag() {
      dataLayer.push(arguments);
    };

    const script = this.document.createElement("script");
    script.async = true;
    script.referrerPolicy = "no-referrer";
    script.src =
      `https://www.googletagmanager.com/gtag/js?id=${encodeURIComponent(this.measurementId)}`;
    script.dataset.rtsGoogleAnalytics = this.measurementId;
    this.document.head.appendChild(script);

    this.window.gtag("js", new Date());
    this.window.gtag("config", this.measurementId, {
      allow_google_signals: false,
      allow_ad_personalization_signals: false,
      page_location: this._pageLocationWithoutQuery(),
      page_referrer: this._pageReferrerWithoutQuery(),
    });
  }

  _pageLocationWithoutQuery() {
    const location = this.window?.location;
    if (!location) return undefined;
    return `${location.origin || ""}${location.pathname || "/"}`;
  }

  _pageReferrerWithoutQuery() {
    const value = this.document?.referrer;
    if (!value) return "";
    try {
      const url = new URL(value);
      if (url.protocol !== "http:" && url.protocol !== "https:") return "";
      return `${url.origin}${url.pathname || "/"}`;
    } catch {
      return "";
    }
  }
}
