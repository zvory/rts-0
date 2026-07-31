import assert from "node:assert/strict";
import {
  ANALYTICS_CONSENT_STORAGE_KEY,
  AnalyticsConsent,
  analyticsMeasurementId,
} from "../../client/src/analytics_consent.js";

function memoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem(key) { return values.get(key) ?? null; },
    setItem(key, value) { values.set(key, String(value)); },
  };
}

function fakeElement(tagName) {
  return {
    tagName: tagName.toUpperCase(),
    children: [],
    dataset: {},
    listeners: {},
    addEventListener(type, handler) { this.listeners[type] = handler; },
    removeEventListener(type, handler) {
      if (this.listeners[type] === handler) delete this.listeners[type];
    },
    append(...children) { this.children.push(...children); },
    remove() { this.removed = true; },
    setAttribute(name, value) { this[name] = String(value); },
  };
}

function fakeDocument() {
  const head = fakeElement("head");
  head.appendChild = (child) => { head.children.push(child); return child; };
  const body = fakeElement("body");
  body.appendChild = (child) => { body.children.push(child); return child; };
  return {
    head,
    body,
    createElement: fakeElement,
    querySelector() { return null; },
  };
}

{
  const documentObj = {
    querySelector(selector) {
      assert.equal(selector, 'meta[name="rts-google-analytics-id"]');
      return { getAttribute: () => "G-06WVK0QHVR" };
    },
  };
  assert.equal(analyticsMeasurementId(documentObj), "G-06WVK0QHVR");
  assert.equal(analyticsMeasurementId({ querySelector: () => ({ getAttribute: () => "bad" }) }), "");
}

{
  const documentObj = fakeDocument();
  const tracker = new AnalyticsConsent({ documentObj, windowObj: {}, measurementId: "" });
  assert.equal(tracker.start(), false);
  assert.equal(documentObj.body.children.length, 0);
  assert.equal(documentObj.head.children.length, 0);
}

{
  const documentObj = fakeDocument();
  const storage = memoryStorage();
  const tracker = new AnalyticsConsent({
    documentObj,
    windowObj: { location: { origin: "https://bewegungskrieg.net", pathname: "/lab" } },
    storage,
    measurementId: "G-06WVK0QHVR",
  });
  assert.equal(tracker.start(), true);
  assert.equal(documentObj.body.children.length, 1, "undecided visitors see a consent prompt");
  assert.equal(documentObj.head.children.length, 0, "Google does not load before consent");

  tracker.declineButton.listeners.click();
  assert.equal(storage.getItem(ANALYTICS_CONSENT_STORAGE_KEY), "denied");
  assert.equal(documentObj.head.children.length, 0, "declining never loads Google");
}

{
  const documentObj = fakeDocument();
  const storage = memoryStorage();
  const windowObj = {
    location: {
      origin: "https://bewegungskrieg.net",
      pathname: "/",
      search: "?rtsRoom=private-room",
    },
  };
  const tracker = new AnalyticsConsent({
    documentObj,
    windowObj,
    storage,
    measurementId: "G-06WVK0QHVR",
  });
  tracker.start();
  tracker.acceptButton.listeners.click();

  assert.equal(storage.getItem(ANALYTICS_CONSENT_STORAGE_KEY), "granted");
  assert.equal(documentObj.head.children.length, 1);
  assert.equal(
    documentObj.head.children[0].src,
    "https://www.googletagmanager.com/gtag/js?id=G-06WVK0QHVR",
  );
  assert.equal(windowObj.dataLayer[1][0], "config");
  assert.equal(windowObj.dataLayer[1][1], "G-06WVK0QHVR");
  assert.equal(windowObj.dataLayer[1][2].allow_google_signals, false);
  assert.equal(windowObj.dataLayer[1][2].allow_ad_personalization_signals, false);
  assert.equal(
    windowObj.dataLayer[1][2].page_location,
    "https://bewegungskrieg.net/",
    "page URLs sent to Google exclude user-controlled query parameters",
  );
}

{
  const documentObj = fakeDocument();
  const tracker = new AnalyticsConsent({
    documentObj,
    windowObj: { location: { origin: "https://bewegungskrieg.net", pathname: "/" } },
    storage: memoryStorage({ [ANALYTICS_CONSENT_STORAGE_KEY]: "granted" }),
    measurementId: "G-06WVK0QHVR",
  });
  tracker.start();
  assert.equal(documentObj.body.children.length, 0, "remembered consent skips the prompt");
  assert.equal(documentObj.head.children.length, 1, "remembered consent loads the tag once");
  tracker.start();
  assert.equal(documentObj.head.children.length, 1, "restarting cannot duplicate the tag");
}
