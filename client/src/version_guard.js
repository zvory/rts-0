import { S } from "./protocol.js";

const CHECK_INTERVAL_MS = 10000;
const CHECK_TIMEOUT_MS = 3000;
const RELOAD_KEY = "rts.versionGuard.lastReload";
const REFRESH_BUILD_PARAM = "rtsClientBuild";

function knownBuild(value) {
  return typeof value === "string" && value.trim() && value.trim() !== "unknown"
    ? value.trim() : null;
}

/** Only ordinary main/lobby screens can recover; room startup and authoring stay untouched. */
export function canRefreshClient(app, lobbyVisible) {
  return lobbyVisible && !app.match && !app.versionTransitionActive &&
    !app.labLaunch && !app.labCatalogLaunch && !app.labHandoffLaunch &&
    !app.replayLaunch && !app.matchLaunch && !app.snapshotStreamLaunch &&
    !app.stressTestLaunch && !app.devWatch && !app.mapPreviewLaunch;
}

/** Page-owned build handshake and lobby-only browser refresh. */
export class VersionGuard {
  constructor({
    net,
    isLobby,
    clientBuild = document.querySelector('meta[name="rts-client-build"]')?.content,
    document: doc = globalThis.document,
    window: win = globalThis.window,
    fetch: fetchVersion = (...args) => globalThis.fetch(...args),
    reload = (build) => {
      const url = new URL(win.location.href);
      url.searchParams.set(REFRESH_BUILD_PARAM, build);
      // Replacing an identical URL with a fragment can be a same-document
      // navigation. Manual recovery must actually request the document again.
      if (url.href === win.location.href) win.location.reload();
      else win.location.replace(url.href);
    },
  }) {
    this.net = net;
    this.isLobby = isLobby;
    this.clientBuild = knownBuild(clientBuild);
    this.doc = doc;
    this.win = win;
    this.fetchVersion = fetchVersion;
    this.reload = reload;
    this.mismatch = false;
    this.destroyed = false;
    this.refreshing = false;
    this.onBuild = (message) => this.observe(message?.buildId);
    this.onMismatch = () => this.warn("unsupported-snapshot");
    this.onVisible = () => {
      if (!this.doc.hidden) void this.check();
    };
    // A back/forward-cache entry resumes this same page, including its guard.
    this.onPageHide = (event) => {
      if (!event.persisted) this.destroy();
    };
    this.onRefresh = () => this.refresh();
  }

  start({ offline = false } = {}) {
    globalThis.__RTS_BUILD__ = this.clientBuild || "unknown";
    // Once recovery loaded the requested build, the URL marker has done its job.
    // Keep it on stale pages so blocked storage cannot cause a refresh loop.
    const url = new URL(this.win.location.href);
    if (this.clientBuild && url.searchParams.get(REFRESH_BUILD_PARAM) === this.clientBuild) {
      url.searchParams.delete(REFRESH_BUILD_PARAM);
      try {
        this.win.history.replaceState(this.win.history.state, "", url.href);
      } catch {
        // URL cleanup is cosmetic; restricted history must not block version checks.
      }
    }
    if (offline || this.started) return;
    this.started = true;
    this.net.on(S.WELCOME, this.onBuild);
    this.net.on(S.PONG, this.onBuild);
    this.net.on("versionMismatch", this.onMismatch);
    this.doc.addEventListener("visibilitychange", this.onVisible);
    this.win.addEventListener("pagehide", this.onPageHide);
    this.win.addEventListener("pageshow", this.onVisible);
    this.win.addEventListener("focus", this.onVisible);
    this.timer = this.win.setInterval(() => void this.check(), CHECK_INTERVAL_MS);
    void this.check();
  }

  observe(serverBuild) {
    const build = knownBuild(serverBuild);
    if (this.destroyed || !this.clientBuild || !build || build === this.clientBuild) return;
    this.warn(build);
  }

  async check() {
    if (this.destroyed || this.request || this.mismatch || !this.isLobby()) return;
    const request = new AbortController();
    this.request = request;
    const timeout = this.win.setTimeout(() => request.abort(), CHECK_TIMEOUT_MS);
    try {
      const response = await this.fetchVersion("/version", { cache: "no-store", signal: request.signal });
      if (response.ok) this.observe(await response.text());
    } catch {
      // A network failure is not evidence of incompatible versions.
    } finally {
      this.win.clearTimeout(timeout);
      if (this.request === request) this.request = null;
    }
  }

  warn(serverBuild) {
    if (this.destroyed) return;
    if (!this.mismatch) {
      this.mismatch = true;
      this.serverBuild = serverBuild;
    }
    this.update();
  }

  mount() {
    this.el = this.doc.createElement("section");
    this.el.id = "version-mismatch";
    this.el.setAttribute("role", "alert");
    this.el.setAttribute("aria-live", "assertive");
    this.el.setAttribute("aria-labelledby", "version-mismatch-title");
    const panel = this.doc.createElement("div");
    panel.className = "version-mismatch-panel";
    const title = this.doc.createElement("h2");
    title.id = "version-mismatch-title";
    title.textContent = "Version mismatch";
    this.detail = this.doc.createElement("p");
    this.button = this.doc.createElement("button");
    this.button.type = "button";
    this.button.textContent = "Refresh client";
    this.button.addEventListener("click", this.onRefresh);
    panel.append(title, this.detail, this.button);
    this.el.append(panel);
    this.doc.body.append(this.el);
  }

  update() {
    if (!this.mismatch || this.destroyed) return;
    if (!this.isLobby()) {
      if (this.el) this.el.hidden = true;
      if (this.button) this.button.disabled = true;
      return;
    }
    if (!this.el) this.mount();
    this.el.hidden = false;
    this.button.disabled = this.refreshing;
    this.detail.textContent = "A game update is available. Refreshing loads the current client.";
    if (this.refreshing) return;
    // A build-specific URL bypasses a stale document cache and also bounds recovery
    // when browser privacy settings make sessionStorage unavailable.
    if (new URL(this.win.location.href).searchParams.get(REFRESH_BUILD_PARAM) === this.serverBuild) return;
    try {
      const pair = JSON.stringify([this.clientBuild, this.serverBuild]);
      if (this.win.sessionStorage.getItem(RELOAD_KEY) === pair) return;
      this.win.sessionStorage.setItem(RELOAD_KEY, pair);
    } catch {
      // Navigation carries the loop guard even without browser storage.
    }
    this.refresh();
  }

  refresh() {
    // Recheck at click time as a countdown/start can arrive while the notice is visible.
    if (this.destroyed || this.refreshing || !this.isLobby()) return;
    this.refreshing = true;
    this.button.disabled = true;
    this.detail.textContent = "Refreshing client…";
    this.reload(this.serverBuild);
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.win.clearInterval(this.timer);
    this.request?.abort();
    this.net.off(S.WELCOME, this.onBuild);
    this.net.off(S.PONG, this.onBuild);
    this.net.off("versionMismatch", this.onMismatch);
    this.doc.removeEventListener("visibilitychange", this.onVisible);
    this.win.removeEventListener("pagehide", this.onPageHide);
    this.win.removeEventListener("pageshow", this.onVisible);
    this.win.removeEventListener("focus", this.onVisible);
    this.button?.removeEventListener("click", this.onRefresh);
    this.el?.remove();
  }
}
