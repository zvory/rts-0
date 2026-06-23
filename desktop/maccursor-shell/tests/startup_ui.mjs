import assert from "node:assert/strict";

import {
  LAST_PROFILE_KEY,
  initialProfileId,
  profileForId,
  resolveStartupProfiles,
  storeLastProfileId,
} from "../ui/startup.js";

const profiles = [
  {
    id: "beta",
    label: "Beta",
    url: "https://rts-0-zvorygin-beta.fly.dev/",
    summary: "Playtest channel",
  },
  {
    id: "mainline",
    label: "Mainline",
    url: "https://rts-0-zvorygin.fly.dev/",
    summary: "Current public release",
  },
];

function storageStub() {
  const values = new Map();
  return {
    getItem(key) {
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
  };
}

const root = {
  __RTS_DESKTOP_STARTUP: {
    profiles,
    defaultProfileId: "beta",
  },
};

assert.deepEqual(
  resolveStartupProfiles(root).map((profile) => profile.id),
  ["beta", "mainline"],
);
assert.equal(profileForId("beta", profiles)?.url, "https://rts-0-zvorygin-beta.fly.dev/");
assert.equal(profileForId("custom", profiles), null);

const storage = storageStub();
assert.equal(initialProfileId(root, storage, profiles), "beta");
storeLastProfileId(storage, "mainline");
assert.equal(storage.getItem(LAST_PROFILE_KEY), "mainline");
assert.equal(initialProfileId(root, storage, profiles), "mainline");
storeLastProfileId(storage, "local");
assert.equal(initialProfileId(root, storage, profiles), "beta");

assert.deepEqual(resolveStartupProfiles({ __RTS_DESKTOP_STARTUP: { profiles: [] } }), []);
assert.deepEqual(resolveStartupProfiles({ __RTS_DESKTOP_STARTUP: { profiles: [{ id: "beta" }] } }), []);
