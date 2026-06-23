export const LAST_PROFILE_KEY = "rts.desktop.lastProfile";

function validProfile(profile) {
  return (
    profile &&
    typeof profile.id === "string" &&
    typeof profile.label === "string" &&
    typeof profile.url === "string" &&
    typeof profile.summary === "string"
  );
}

export function resolveStartupProfiles(root = globalThis) {
  const profiles = root?.__RTS_DESKTOP_STARTUP?.profiles;
  if (!Array.isArray(profiles)) return [];
  return profiles.filter(validProfile);
}

export function profileForId(profileId, profiles) {
  return profiles.find((profile) => profile.id === profileId) || null;
}

export function loadLastProfileId(storage) {
  try {
    return storage?.getItem(LAST_PROFILE_KEY) || null;
  } catch {
    return null;
  }
}

export function storeLastProfileId(storage, profileId) {
  try {
    storage?.setItem(LAST_PROFILE_KEY, profileId);
  } catch {}
}

export function initialProfileId(
  root = globalThis,
  storage = root?.localStorage,
  profiles = resolveStartupProfiles(root),
) {
  const remembered = loadLastProfileId(storage);
  if (remembered && profileForId(remembered, profiles)) return remembered;
  const defaultProfileId = root?.__RTS_DESKTOP_STARTUP?.defaultProfileId;
  if (defaultProfileId && profileForId(defaultProfileId, profiles)) return defaultProfileId;
  return profiles[0]?.id || null;
}

function setStatus(statusEl, message, state = "") {
  if (!statusEl) return;
  statusEl.textContent = message;
  if (state) statusEl.dataset.state = state;
  else delete statusEl.dataset.state;
}

function createProfileButton(doc, root, profile, selectedId, statusEl) {
  const button = doc.createElement("button");
  button.type = "button";
  button.className = "profile-option";
  button.dataset.profileId = profile.id;
  button.setAttribute("aria-pressed", profile.id === selectedId ? "true" : "false");

  const title = doc.createElement("p");
  title.className = "profile-name";
  title.textContent = profile.label;

  const summary = doc.createElement("span");
  summary.className = "profile-summary";
  summary.textContent = profile.summary;

  const url = doc.createElement("p");
  url.className = "profile-url";
  url.textContent = new URL(profile.url).hostname;

  const action = doc.createElement("span");
  action.className = "profile-action";
  action.textContent = "Open";

  button.append(title, summary, url, action);
  button.addEventListener("click", () => {
    storeLastProfileId(root?.localStorage, profile.id);
    setStatus(statusEl, `Opening ${profile.label}...`);
    root.location.assign(profile.url);
  });
  return button;
}

export function initStartup(doc = document, root = globalThis) {
  const list = doc.getElementById("profile-list");
  const status = doc.getElementById("startup-status");
  const profiles = resolveStartupProfiles(root);
  const selectedId = initialProfileId(root, root?.localStorage, profiles);

  if (!list) return;
  list.replaceChildren();

  if (profiles.length === 0) {
    setStatus(status, "Desktop startup metadata is unavailable.", "error");
    return;
  }

  for (const profile of profiles) {
    list.append(createProfileButton(doc, root, profile, selectedId, status));
  }
  setStatus(status, "");
}

if (typeof document !== "undefined") {
  initStartup();
}
