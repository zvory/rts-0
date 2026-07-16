import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const shellRoot = new URL("../", import.meta.url);
const tauriConfig = JSON.parse(
  await readFile(new URL("src-tauri/tauri.conf.json", shellRoot), "utf8"),
);
const windowsLauncher = await readFile(new URL("run.cmd", shellRoot), "utf8");
const windowsArtifactBuilder = await readFile(
  new URL("build-unsigned-windows.mjs", shellRoot),
  "utf8",
);

assert.equal(tauriConfig.productName, "Bewegungskrieg");
assert.equal(tauriConfig.identifier, "dev.bewegungskrieg.Bewegungskrieg");
assert.equal(tauriConfig.build.frontendDist, "../ui");
assert.equal("externalBin" in tauriConfig.bundle, false);
assert.equal("resources" in tauriConfig.bundle, false);
assert.match(windowsLauncher, /cargo run --manifest-path/);
assert.match(windowsLauncher, /CARGO_TARGET_DIR=%LOCALAPPDATA%\\rts-0\\tauri-target-windows/);
assert.match(windowsLauncher, /CARGO_BUILD_JOBS=2/);
assert.doesNotMatch(windowsLauncher, /rts-server|server\.exe/i);
assert.match(windowsArtifactBuilder, /"--bundles",\s*"nsis"/);
assert.match(windowsArtifactBuilder, /unsigned-windows-nsis/);
assert.match(windowsArtifactBuilder, /rts-server\.exe/);
assert.doesNotMatch(windowsArtifactBuilder, /targets:\s*"msi"/);

import {
  LAST_PROFILE_KEY,
  formatStartupFailure,
  initialProfileId,
  invalidStartupProfiles,
  openProfile,
  profileForId,
  resolveStartupProfiles,
  startupFailureFromLocation,
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
assert.deepEqual(
  invalidStartupProfiles({
    __RTS_DESKTOP_STARTUP: {
      profiles: [{ id: "bad", label: "Bad", url: "not a url", summary: "Broken" }],
    },
  }).map((profile) => profile.id),
  ["bad"],
);
assert.deepEqual(
  resolveStartupProfiles({
    __RTS_DESKTOP_STARTUP: {
      profiles: [{ id: "bad", label: "Bad", url: "not a url", summary: "Broken" }],
    },
  }),
  [],
);

const failure = startupFailureFromLocation({
  href: "tauri://localhost/index.html?failure=load-timeout&message=Timed%20out&url=https%3A%2F%2Frts.example%2F",
});
assert.deepEqual(failure, {
  code: "load-timeout",
  message: "Timed out",
  url: "https://rts.example/",
});
assert.equal(formatStartupFailure(failure), "Timed out (https://rts.example/)");

const invoked = [];
await openProfile(
  {
    __TAURI_INTERNALS__: {
      invoke(command, payload) {
        invoked.push({ command, payload });
        return Promise.resolve();
      },
    },
  },
  profiles[0],
);
assert.deepEqual(invoked, [{ command: "desktop_open_profile", payload: { profileId: "beta" } }]);
