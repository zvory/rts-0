export const LAST_PROFILE_KEY = "rts.desktop.lastProfile";

function profileHasRequiredShape(profile) {
  return (
    profile &&
    typeof profile.id === "string" &&
    typeof profile.label === "string" &&
    typeof profile.url === "string" &&
    typeof profile.summary === "string"
  );
}

function parseProfileUrl(value) {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

export function invalidStartupProfiles(root = globalThis) {
  const profiles = root?.__RTS_DESKTOP_STARTUP?.profiles;
  if (!Array.isArray(profiles)) return [];
  return profiles.filter((profile) => profileHasRequiredShape(profile) && !parseProfileUrl(profile.url));
}

export function resolveStartupProfiles(root = globalThis) {
  const profiles = root?.__RTS_DESKTOP_STARTUP?.profiles;
  if (!Array.isArray(profiles)) return [];
  return profiles.filter((profile) => profileHasRequiredShape(profile) && parseProfileUrl(profile.url));
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

function tauriInvoke(root = globalThis) {
  const candidates = [
    root?.__TAURI_INTERNALS__?.invoke,
    root?.__TAURI__?.core?.invoke,
    root?.__TAURI__?.tauri?.invoke,
    root?.__TAURI__?.invoke,
  ];
  return candidates.find((candidate) => typeof candidate === "function") || null;
}

async function invokeDesktop(root, command, payload = {}) {
  const invoke = tauriInvoke(root);
  if (!invoke) throw new Error("Desktop bridge is unavailable.");
  return await invoke(command, payload);
}

export async function reportDesktopEvent(root, event, message = "", url = "") {
  try {
    await invokeDesktop(root, "desktop_log_client_event", { event, message, url });
  } catch {}
}

export async function openProfile(root, profile) {
  const invoke = tauriInvoke(root);
  if (invoke) {
    await invoke("desktop_open_profile", { profileId: profile.id });
    return;
  }
  root.location.assign(profile.url);
}

export function startupFailureFromLocation(location) {
  if (!location?.href) return null;
  let url;
  try {
    url = new URL(location.href);
  } catch {
    return null;
  }
  const code = url.searchParams.get("failure");
  if (!code) return null;
  return {
    code,
    message:
      url.searchParams.get("message") ||
      "The desktop shell could not open the selected release channel.",
    url: url.searchParams.get("url") || "",
  };
}

export function formatStartupFailure(failure) {
  if (!failure) return "";
  return failure.url ? `${failure.message} (${failure.url})` : failure.message;
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
  url.textContent = parseProfileUrl(profile.url)?.hostname || profile.url;

  const action = doc.createElement("span");
  action.className = "profile-action";
  action.textContent = "Open";

  button.append(title, summary, url, action);
  button.addEventListener("click", () => {
    storeLastProfileId(root?.localStorage, profile.id);
    setStatus(statusEl, `Opening ${profile.label}...`);
    void openProfile(root, profile).catch((err) => {
      const message = err?.message || String(err);
      setStatus(statusEl, `Could not open ${profile.label}: ${message}`, "error");
      void reportDesktopEvent(root, "startup_profile_open_failed", message, profile.url);
    });
  });
  return button;
}

async function refreshLogInfo(doc, root) {
  const actions = doc.getElementById("diagnostics-actions");
  const logPath = doc.getElementById("log-path");
  try {
    const info = await invokeDesktop(root, "desktop_log_info");
    if (actions) actions.hidden = false;
    if (logPath) {
      logPath.hidden = false;
      logPath.textContent = `Log file: ${info.logFile}`;
    }
    return info;
  } catch {
    if (actions) actions.hidden = true;
    if (logPath) logPath.hidden = true;
    return null;
  }
}

function initLogActions(doc, root, statusEl) {
  const copyButton = doc.getElementById("copy-log-path");
  const revealButton = doc.getElementById("reveal-logs");
  if (!copyButton && !revealButton) return;

  copyButton?.addEventListener("click", () => {
    void (async () => {
      const info = await refreshLogInfo(doc, root);
      if (!info) {
        setStatus(statusEl, "Log path is unavailable in this window.", "error");
        return;
      }
      try {
        if (typeof root?.navigator?.clipboard?.writeText !== "function") {
          throw new Error("Clipboard unavailable.");
        }
        await root.navigator.clipboard.writeText(info.logFile);
        setStatus(statusEl, "Log path copied.");
      } catch {
        setStatus(statusEl, info.logFile);
      }
    })();
  });

  revealButton?.addEventListener("click", () => {
    void invokeDesktop(root, "desktop_reveal_logs")
      .then(() => setStatus(statusEl, "Opening log folder."))
      .catch((err) => setStatus(statusEl, err?.message || String(err), "error"));
  });

  void refreshLogInfo(doc, root);
}

export function initStartup(doc = document, root = globalThis) {
  const list = doc.getElementById("profile-list");
  const status = doc.getElementById("startup-status");
  const failure = startupFailureFromLocation(root?.location);
  const invalidProfiles = invalidStartupProfiles(root);
  const profiles = resolveStartupProfiles(root);
  const selectedId = initialProfileId(root, root?.localStorage, profiles);

  if (!list) return;
  list.replaceChildren();
  initLogActions(doc, root, status);

  if (profiles.length === 0) {
    const message =
      invalidProfiles.length > 0
        ? "Desktop release-channel configuration is invalid."
        : "Desktop startup metadata is unavailable.";
    setStatus(status, message, "error");
    void reportDesktopEvent(root, "startup_profiles_unavailable", message);
    return;
  }

  for (const profile of profiles) {
    list.append(createProfileButton(doc, root, profile, selectedId, status));
  }
  if (invalidProfiles.length > 0) {
    const message = "One release channel has an invalid URL and was hidden.";
    setStatus(status, message, "error");
    void reportDesktopEvent(root, "startup_profile_invalid", message);
  } else if (failure) {
    setStatus(status, formatStartupFailure(failure), "error");
  } else {
    setStatus(status, "");
  }
}

if (typeof document !== "undefined") {
  initStartup();
}
