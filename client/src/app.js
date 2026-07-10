// Bootstrap & wiring. See docs/design/client-ui.md §4.1 (module contracts) and §4.
//
// This is the only place that knows about *all* the modules. It owns:
//   - the WebSocket lifecycle (via Net),
//   - the lobby <-> game screen transition,
//   - constructing the in-game modules from the `start` payload, and
//   - the requestAnimationFrame render loop (with snapshot interpolation).
//
// Everything below codes strictly against the export signatures in docs/design/client-ui.md §4.1;
// the modules themselves are owned by other files. Keep this layer thin: it should
// orchestrate, not implement game logic.

import { Net } from "./net.js";
import { Lobby } from "./lobby.js";
import { BranchStaging } from "./branch_staging.js";
import { Audio, SOUND_MANIFEST } from "./audio.js";
import { S } from "./protocol.js";
import { TOAST_MS } from "./alerts.js";
import {
  buildLabLaunchConfig,
  labCatalogRouteConfig,
  devWatchConfig,
  diagnostics,
  dom,
  formatScore,
  labLaunchConfig,
  replayLaunchConfig,
  wsUrl,
} from "./bootstrap.js";
import { Match } from "./match.js";
import { MatchHistory } from "./match_history.js";
import { applyMatchUnitRanges } from "./match_settings_toggles.js";
import { readPredictionEnabled, writePredictionEnabled } from "./prediction_settings.js";
import { readUnitRangesEnabled, writeUnitRangesEnabled } from "./unit_range_settings.js";
import { createObserverAnalysisOverlayPreferences } from "./observer_analysis_overlay.js";
import { createAiDiagnosticsPanelPreferences } from "./ai_diagnostics_panel.js";
import { ReplayViewer } from "./replay_viewer.js";
import { createRoomCapabilities } from "./room_capabilities.js";
import { selectInitialCameraView } from "./camera_view_selection.js";
import { matchLaunchConfig, nextMatchLaunchAction } from "./launch_url.js";
import { CAMERA } from "./config.js";
import { formatTeamLabel, scoreRowIsWinner } from "./scoreboard.js";
import { StatusBadge } from "./status_badge.js";
import {
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "./hotkey_profiles.js";
import { buildCommandCardContextCatalog } from "./hud_command_card.js";
import { LabClient } from "./lab_client.js";
import { LabCatalogScreen } from "./lab_catalog.js";
import { createDefaultControlPolicy, createLabControlPolicy } from "./lab_control_policy.js";
import { LabPanel } from "./lab_panel.js";
import { LabMapEditorSession } from "./lab_map_editor_session.js";
import { applyLabMapReset } from "./lab_map_reset.js";
import {
  fetchLabScenarioSubmissionCapability as fetchLabScenarioSubmissionCapabilityRequest,
} from "./lab_scenario_submission_capability.js";
import { SettingsContainer } from "./settings_container.js";
import { buildSettingsTabs } from "./settings_panels.js";
import { resolveVisualProfileLaunch } from "./visual_profiles.js";

/**
 * App-level heartbeat interval (ms). The server drops connections idle for 40s,
 * so we ping well inside that window to keep a healthy connection alive.
 */
const HEARTBEAT_MS = 15000;

export function isLivePlayerMatch(match) {
  return !!match &&
    !!match.state &&
    match.running !== false &&
    !match.state.spectator &&
    !match.replayViewer &&
    !match.labMetadata;
}

export function shouldWarnBeforeUnload({
  match = null,
  allowUnloadWithoutWarning = false,
} = {}) {
  return !allowUnloadWithoutWarning && isLivePlayerMatch(match);
}

/**
 * The whole application. A single instance is created on load. It can host
 * many sequential matches: a live match can roll straight into post-match replay
 * playback, then tear down cleanly when the user returns to lobby, all on the
 * same Net connection.
 */
export class App {
  constructor() {
    /** @type {Net} persistent connection across lobby + matches. */
    this.net = new Net(wsUrl(), diagnostics);
    this.devWatch = devWatchConfig();
    this.labCatalogLaunch = labCatalogRouteConfig();
    this.labLaunch = labLaunchConfig();
    this.labVisualProfileState = resolveVisualProfileLaunch(this.labLaunch || this.labCatalogLaunch);
    this.replayLaunch = replayLaunchConfig();
    this.matchLaunch = matchLaunchConfig();
    /**
     * Audio engine. Long-lived across matches: the AudioContext is unlocked
     * by the user's first gesture (anywhere in the page), and we want that
     * unlock to survive lobby->match->lobby transitions.
     * @type {Audio}
     */
    this.audio = new Audio();
    void this.audio.preload(SOUND_MANIFEST);
    this.statusBadge = new StatusBadge(dom.version);
    this.hotkeyProfiles = new HotkeyProfileService({
      catalog: buildHotkeyCommandCatalog(buildCommandCardContextCatalog()),
    });
    globalThis.rtsHotkeys = this.hotkeyProfiles;
    this.settings = new SettingsContainer({
      button: dom.settingsButton,
      menu: dom.settingsMenu,
    });
    /** @type {Lobby} */
    this.lobby = new Lobby(dom.lobbyScreen, this.net, this.audio);
    this.branchStaging = new BranchStaging(dom.branchScreen, this.net);
    /** @type {MatchHistory|null} Lazy-init when the lobby first shows. */
    this.matchHistory = null;
    /** @type {Match|null} the currently running match, if any. */
    this.match = null;
    this.labCatalog = null;
    this.labClient = null;
    this.labPanel = null;
    this.labMapEditorSession = null;
    this.labControlPolicy = null;
    /** @type {number|undefined} pending toast hide timer. */
    this.toastTimer = undefined;
    /** @type {number|undefined} heartbeat interval id while connected. */
    this.heartbeatTimer = undefined;
    /** Whether the WebSocket has ever reached open in this page session. */
    this.hasConnected = false;

    // Bind handlers once so we can off() them symmetrically.
    this.onStart = this.onStart.bind(this);
    this.onError = this.onError.bind(this);
    this.onObservationReady = this.onObservationReady.bind(this);
    this.onGameOver = this.onGameOver.bind(this);
    this.onShutdownWarning = this.onShutdownWarning.bind(this);
    this.onBackToLobby = this.onBackToLobby.bind(this);
    this.onCloseScorePanel = this.onCloseScorePanel.bind(this);
    this.onGameOverOverlayClick = this.onGameOverOverlayClick.bind(this);
    this.onOpen = this.onOpen.bind(this);
    this.onClose = this.onClose.bind(this);
    this.onLobbyForMatchLaunch = this.onLobbyForMatchLaunch.bind(this);
    this.onBranchFromTickCreated = this.onBranchFromTickCreated.bind(this);
    this.onBeforeUnload = this.onBeforeUnload.bind(this);
    this.inReplayPlayback = false;
    this.allowUnloadWithoutWarning = false;
    this.pendingCameraView = null;
    this.predictionEnabled = readPredictionEnabled();
    this.unitRangesEnabled = readUnitRangesEnabled();
    this.observerAnalysisOverlayPreferences = createObserverAnalysisOverlayPreferences();
    this.aiDiagnosticsPanelPreferences = createAiDiagnosticsPanelPreferences();
    this.matchLaunchDone = false;
    this.matchLaunchFailed = false;
    /** AI observation id received at match resolution and retained through replay playback. */
    this.lastObservationRunId = "";
    this.mountLobbySettings();
    if (this.labCatalogLaunch) this.lobby.hide();
  }

  /** Connect, wire global server messages, and show the lobby. */
  async start() {
    this.net.on(S.START, this.onStart);
    this.net.on(S.ERROR, this.onError);
    this.net.on(S.OBSERVATION_READY, this.onObservationReady);
    this.net.on(S.GAME_OVER, this.onGameOver);
    this.net.on(S.BRANCH_FROM_TICK_CREATED, this.onBranchFromTickCreated);
    this.net.on(S.SHUTDOWN_WARNING, this.onShutdownWarning);
    this.net.on(S.LOBBY, this.onLobbyForMatchLaunch);
    this.net.on("open", this.onOpen);
    this.net.on("close", this.onClose);
    dom.gameOverButton.addEventListener("click", this.onBackToLobby);
    dom.gameOverClose?.addEventListener("click", this.onCloseScorePanel);
    dom.gameOver.addEventListener("click", this.onGameOverOverlayClick);
    window.addEventListener("beforeunload", this.onBeforeUnload);

    void this.loadVersion();
    if (this.labCatalogLaunch) {
      this.showLabCatalog();
    } else {
      this.lobby.show();
      this.mountLobbySettings();
      this._mountMatchHistory();
    }
    this.applyDevBanner();
    try {
      await this.net.connect();
      if (this.replayLaunch) this.maybeAutoJoinReplay();
      else if (this.labLaunch) this.maybeAutoJoinLab();
      else if (this.labCatalogLaunch) this.labCatalog?.setConnected(true);
      else if (this.matchLaunch) this.maybeAutoJoinMatchLaunch();
      else this.maybeAutoJoinDevWatch();
    } catch (err) {
      this.showConnectionWarning();
    }
  }

  applyDevBanner() {
    if (!dom.devBanner) return;
    if (!this.devWatch) {
      dom.devBanner.hidden = true;
      return;
    }
    dom.devBanner.textContent = this.devWatch.banner;
    dom.devBanner.hidden = false;
  }

  maybeAutoJoinDevWatch() {
    if (!this.devWatch) return;
    const name = "Spectator";
    if (this.lobby?.elName) this.lobby.elName.value = name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.devWatch.room;
    this.net.join(name, this.devWatch.room, true, this.devWatch.kind === "replay");
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    this.lobby.setStatus("Starting local scenario...");
  }

  maybeAutoJoinReplay() {
    const name = "Spectator";
    if (this.lobby?.elName) this.lobby.elName.value = name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.replayLaunch.room;
    if (this.replayLaunch?.staging) {
      this.lobby?.joinReplayLobby(this.replayLaunch.room);
      return;
    }
    this.net.join(name, this.replayLaunch.room, true, true);
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    this.lobby.setStatus("Starting replay...");
  }

  maybeAutoJoinLab() {
    const name = "Lab Operator";
    if (this.lobby?.elName) this.lobby.elName.value = name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.labLaunch.room;
    this.net.join(name, this.labLaunch.room, true, false);
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    if (this.labCatalogLaunch) this.labCatalog?.setStatus("Starting lab...");
    else this.lobby.setStatus("Starting lab...");
    this.showLabVisualProfileNotice();
  }

  maybeAutoJoinMatchLaunch() {
    if (!this.matchLaunch) return;
    if (this.matchLaunch.errors?.length) {
      this.failMatchLaunch(`Launch URL invalid: ${this.matchLaunch.errors.join(" ")}`);
      return;
    }
    if (this.lobby?.elName) this.lobby.elName.value = this.matchLaunch.name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.matchLaunch.room;
    this.net.join(
      this.matchLaunch.name,
      this.matchLaunch.room,
      this.matchLaunch.spectator,
      false,
    );
    this.lobby.setStatus(`Preparing AI self-play "${this.matchLaunch.room}"...`);
  }

  onLobbyForMatchLaunch(payload) {
    if (!this.matchLaunch || this.matchLaunchDone || this.matchLaunchFailed) return;
    const action = nextMatchLaunchAction(this.matchLaunch, payload, this.net.playerId);
    this.applyMatchLaunchAction(action);
  }

  applyMatchLaunchAction(action) {
    switch (action?.type) {
      case "none":
        return;
      case "wait":
        this.lobby?.setStatus(action.message || "Preparing AI self-play...");
        return;
      case "fail":
        this.failMatchLaunch(action.message || "Launch URL automation failed.");
        return;
      case "setSpectator":
        this.lobby?.setStatus("Setting observer role...");
        this.net.setSpectator(!!action.spectator);
        return;
      case "selectMap":
        this.lobby?.setStatus(`Selecting map "${action.map}"...`);
        this.net.selectMap(action.map);
        return;
      case "addAi":
        this.lobby?.setStatus("Adding AI opponent...");
        this.net.addAi(action.teamId, action.aiProfileId);
        return;
      case "setTeam":
        this.lobby?.setStatus("Assigning AI team...");
        this.net.setTeam(action.id, action.teamId);
        return;
      case "setAiProfile":
        this.lobby?.setStatus("Selecting AI profile...");
        this.net.setAiProfile(action.id, action.aiProfileId);
        return;
      case "ready":
        this.lobby?.setStatus("Ready check...");
        this.net.ready(!!action.ready);
        return;
      case "start":
        this.matchLaunchDone = true;
        this.lobby?.setStatus("Starting AI self-play...");
        this.net.start();
        return;
      case "done":
        this.matchLaunchDone = true;
        this.lobby?.setStatus(action.message || "Launch lobby is ready.");
        return;
      default:
        return;
    }
  }

  failMatchLaunch(message) {
    this.matchLaunchFailed = true;
    this.lobby?.setStatus(message, true);
    this.showToast(message, 10000);
  }

  showLabVisualProfileNotice() {
    const launch = this.labLaunch || this.labCatalogLaunch;
    if (!launch?.visualProfileId && !launch?.visualProfileError) return;
    const state = this.labVisualProfileState || { profile: null, error: null };
    if (state.error) {
      this.showToast(state.error.message, 10000);
      return;
    }
    if (state.profile) {
      this.showToast(`Visual profile: ${state.profile.label || state.profile.id}`, 5000);
    }
  }

  showLabCatalog() {
    if (dom.lobbyScreen) dom.lobbyScreen.hidden = true;
    if (!dom.labEntryScreen) return;
    dom.labEntryScreen.hidden = false;
    if (this.labCatalog) return;
    this.labCatalog = new LabCatalogScreen({
      root: dom.labEntryScreen,
      initialRoom: this.labCatalogLaunch?.room || "default",
      onStart: (selection) => {
        this.labLaunch = {
          ...buildLabLaunchConfig({
            ...selection,
            visualProfile: this.labCatalogLaunch?.visualProfileId || "",
          }),
          visualProfileError: this.labCatalogLaunch?.visualProfileError || null,
        };
        this.labVisualProfileState = resolveVisualProfileLaunch(this.labLaunch);
        this.maybeAutoJoinLab();
      },
    });
    this.labCatalog.mount();
    this.showLabVisualProfileNotice();
  }

  /**
   * Server rejected something (bad join, room full, illegal start, ...).
   * Surface it to the player; the lobby is the most likely context.
   * @param {{msg: string}} m
   */
  onError(m) {
    const msg = m && m.msg ? m.msg : "Server error";
    this.showToast(msg);
    this.labCatalog?.setStatus(msg, { error: true });
  }

  /**
   * Server is draining for deploy. Surface the platform deadline and keep it visible in the
   * lobby status too, because new match starts are disabled while active matches wind down.
   * @param {{deadlineUnixMs?: number, secondsRemaining?: number}} m
   */
  onShutdownWarning(m) {
    const seconds = this.shutdownSecondsRemaining(m);
    const text =
      seconds > 0
        ? `Server deploy in ${this.formatDuration(seconds)}. New matches are disabled.`
        : "Server deploy in progress. New matches are disabled.";
    this.showToast(text, 8000);
    if (this.lobby) this.lobby.setStatus(text, true);
  }

  /**
   * Socket opened: start an app-level heartbeat so a healthy connection is never
   * dropped by the server's idle timeout. We only ping once the socket is open,
   * so we never spam pings before then.
   */
  onOpen() {
    this.hasConnected = true;
    this.stopHeartbeat();
    this.heartbeatTimer = window.setInterval(() => this.net.ping(), HEARTBEAT_MS);
    this.labCatalog?.setConnected(true);
  }

  /** Socket closed: stop the heartbeat so we don't leak the interval. */
  onClose() {
    this.stopHeartbeat();
    const text = this.hasConnected
      ? "Server connection lost. Refresh when the server is available."
      : "Unable to connect to the server. Make sure it is running, then refresh.";
    this.labCatalog?.setConnected(false);
    this.showConnectionWarning(text);
  }

  /** Clear the heartbeat interval if one is running. Idempotent. */
  stopHeartbeat() {
    if (this.heartbeatTimer !== undefined) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
  }

  /**
   * Match is beginning. Build the game-side modules from the static `start`
   * payload, swap screens, and kick off the render loop.
   * @param {object} payload §2.3 start payload
   */
  onStart(payload) {
    diagnostics.mark("app.onStart.begin", {
      map: payload?.map ? `${payload.map.width}x${payload.map.height}` : undefined,
      terrain: payload?.map?.terrain?.length,
      resources: payload?.map?.resources?.length,
      players: payload?.players?.length,
      spectator: payload?.spectator,
    });
    const startsReplay = !!payload?.replay;
    if (!startsReplay) this.lastObservationRunId = "";
    const preserveScorePanel = startsReplay && !dom.gameOver.hidden;
    const capabilities = createRoomCapabilities({ startPayload: payload });
    const labMetadata = payload?.lab || null;
    const visualProfile = labMetadata ? this.labVisualProfileState?.profile || null : null;
    const visualProfileError = labMetadata ? this.labVisualProfileState?.error || null : null;
    const scenarioInitialCamera = labMetadata?.initialCamera || null;

    const carriedCamera = selectInitialCameraView({
      currentView: this.takeMatchCameraView(),
      pendingView: this.pendingCameraView,
      visualProfileView: visualProfile?.initialCamera,
      scenarioView: scenarioInitialCamera,
    });
    this.pendingCameraView = null;

    // If a previous match somehow lingers, tear it down first.
    if (this.match) this.match.destroy();
    this.destroyLabShell();
    this.inReplayPlayback = startsReplay;

    dom.gameScreen.classList.remove("branch-background");
    dom.lobbyScreen.hidden = true;
    if (dom.labEntryScreen) dom.labEntryScreen.hidden = true;
    this.branchStaging.hide();
    if (dom.devLinks) dom.devLinks.hidden = true;
    dom.gameScreen.hidden = false;
    if (!preserveScorePanel) {
      dom.gameOver.hidden = true;
      this.clearScoreboard();
    }

    const MatchClass = startsReplay ? ReplayViewer : Match;
    if (labMetadata) {
      this.labMapEditorSession ||= new LabMapEditorSession();
      this.labClient = new LabClient(this.net);
      this.labClient.setInitialState(labMetadata);
      this.labControlPolicy = createLabControlPolicy({
        labClient: this.labClient,
        metadata: labMetadata,
      });
    } else {
      this.labControlPolicy = createDefaultControlPolicy();
    }
    this.match = new MatchClass(
      this.net,
      payload,
      (msg) => this.showToast(msg),
      this.devWatch,
      this.audio,
      this.statusBadge,
      diagnostics,
      {
        initialCamera: carriedCamera,
        hotkeyProfiles: this.hotkeyProfiles,
        settings: this.settings,
        onBackToLobby: this.onBackToLobby,
        predictionEnabled: this.predictionEnabled,
        unitRangesEnabled: this.unitRangesEnabled,
        onPredictionEnabledChange: (enabled) => this.setPredictionEnabled(enabled),
        onUnitRangesEnabledChange: (enabled) => this.setUnitRangesEnabled(enabled),
        observerAnalysisOverlayPreferences: this.observerAnalysisOverlayPreferences,
        aiDiagnosticsPanelPreferences: this.aiDiagnosticsPanelPreferences,
        capabilities,
        cameraMaxZoom: labMetadata ? CAMERA.labMaxZoom : undefined,
        labMetadata,
        labClient: this.labClient,
        labControlPolicy: this.labControlPolicy,
        visualProfile,
        visualProfileError,
        onLabToolChange: (change) => this.labPanel?.applyLabToolChange?.(change),
      },
    );
    if (labMetadata) {
      this.labPanel = new LabPanel({
        root: dom.gameScreen,
        labClient: this.labClient,
        launch: this.labLaunch,
        startPayload: payload,
        match: this.match,
        mapEditorSession: this.labMapEditorSession,
        applyLabMapReset: (outcome) => applyLabMapReset(this.match, outcome),
        submissionCapability: this.fetchLabScenarioSubmissionCapability(),
        openWindow: (url) => window.open(url, "_blank", "noopener,noreferrer"),
      });
    }
    diagnostics.mark("app.onStart.end");
  }

  async fetchLabScenarioSubmissionCapability() {
    return fetchLabScenarioSubmissionCapabilityRequest();
  }

  onBranchFromTickCreated(m) {
    const branchRoom = (m?.branchRoom || "").trim();
    if (!branchRoom) return;
    if (this.match) {
      this.pendingCameraView = this.takeMatchCameraView();
      if (typeof this.match.freezeForBranchStagingBackground === "function") {
        this.match.freezeForBranchStagingBackground();
      } else {
        this.match.destroy();
        this.match = null;
      }
    }
    this.inReplayPlayback = false;
    this.statusBadge.clearMatchMetrics();
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = false;
    dom.gameScreen.classList.add("branch-background");
    dom.lobbyScreen.hidden = true;
    if (dom.devLinks) dom.devLinks.hidden = true;
    this.branchStaging.show();

    const name = (this.lobby?.elName && this.lobby.elName.value.trim()) || "Commander";
    if (this.lobby?.elRoom) this.lobby.elRoom.value = branchRoom;
    this.net.join(name, branchRoom, true, true);
  }

  takeMatchCameraView() {
    if (!this.match || typeof this.match.cameraView !== "function") return null;
    const view = this.match.cameraView();
    return Number.isFinite(view?.x) && Number.isFinite(view?.y) && Number.isFinite(view?.zoom)
      ? view
      : null;
  }

  /**
   * Match resolved. Show the overlay with the right verdict; the button
   * (wired in start()) returns to the lobby and tears the match down.
   * @param {{winnerId: number|null, winnerTeamId?: number|null, you: "won"|"lost"|"draw"}} m
   */
  onGameOver(m) {
    const verdict = m && m.you ? m.you : "draw";
    const text =
      verdict === "won" ? "Victory" : verdict === "lost" ? "Defeat" : "Draw";
    if (verdict === "won") this.audio.play("victory", { category: "ui", priority: 5 });
    else if (verdict === "lost") this.audio.play("defeat", { category: "ui", priority: 5 });
    dom.gameOverText.textContent = text;
    dom.gameOverText.dataset.verdict = verdict; // lets CSS tint win/lose/draw
    this.renderScoreboard(
      Array.isArray(m?.scores) ? m.scores : [],
      m?.winnerId ?? null,
      m?.winnerTeamId ?? null,
    );
    this.renderObservationId(this.lastObservationRunId);
    dom.gameOver.hidden = false;
    // Freeze the loop but keep the final frame visible behind the overlay.
    if (this.match) this.match.stop();
  }

  onObservationReady(m) {
    this.lastObservationRunId = typeof m?.matchRunId === "string" ? m.matchRunId.trim() : "";
    if (!dom.gameOver.hidden) this.renderObservationId(this.lastObservationRunId);
  }

  /**
   * Render the frozen score snapshot carried by the gameOver message.
   * @param {Array<object>} scores
   * @param {number|null} winnerId
   * @param {number|null} winnerTeamId
   */
  renderScoreboard(scores, winnerId, winnerTeamId = null) {
    const root = dom.gameOverScores;
    if (!root) return;
    root.replaceChildren();
    if (!scores.length) {
      root.hidden = true;
      return;
    }

    const table = document.createElement("table");
    table.className = "score-table";
    const thead = document.createElement("thead");
    const header = document.createElement("tr");
    const columns = [
      ["player", "Player"],
      ["teamId", "Team"],
      ["unitScore", "Unit score"],
      ["structureScore", "Structure score"],
      ["unitsKilled", "Units killed"],
      ["unitsLost", "Units lost"],
      ["buildingsKilled", "Buildings killed"],
      ["buildingsLost", "Buildings lost"],
    ];
    for (const [, label] of columns) {
      const th = document.createElement("th");
      th.scope = "col";
      th.textContent = label;
      header.appendChild(th);
    }
    thead.appendChild(header);
    table.appendChild(thead);

    const tbody = document.createElement("tbody");
    for (const score of scores) {
      const tr = document.createElement("tr");
      const id = Number(score?.id);
      if (Number.isFinite(id) && id === this.net.playerId) tr.classList.add("you");
      if (scoreRowIsWinner(score, winnerId, winnerTeamId)) {
        tr.classList.add("winner");
      }

      const player = document.createElement("td");
      player.className = "score-player";
      const swatch = document.createElement("span");
      swatch.className = "score-swatch";
      swatch.style.backgroundColor = typeof score?.color === "string" ? score.color : "#888";
      const name = document.createElement("span");
      name.className = "score-name";
      name.textContent = score?.name || (Number.isFinite(id) ? `Player ${id}` : "Player");
      player.append(swatch, name);
      tr.appendChild(player);

      const team = document.createElement("td");
      team.className = "score-team";
      team.textContent = formatTeamLabel(score?.teamId);
      tr.appendChild(team);

      for (const [key] of columns.slice(2)) {
        const td = document.createElement("td");
        td.className = "score-number";
        td.textContent = formatScore(score?.[key]);
        tr.appendChild(td);
      }
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    root.appendChild(table);
    root.hidden = false;
  }

  clearScoreboard() {
    if (!dom.gameOverScores) return;
    dom.gameOverScores.replaceChildren();
    dom.gameOverScores.hidden = true;
    this.renderObservationId("");
  }

  renderObservationId(matchRunId) {
    if (!dom.gameOverObservation) return;
    const id = typeof matchRunId === "string" ? matchRunId.trim() : "";
    dom.gameOverObservation.hidden = !id;
    dom.gameOverObservation.textContent = id
      ? `Observation ID: ${id}. Share it to retrieve this replay and its server lag logs.`
      : "";
  }

  /** "Back to lobby" button: tear down the match and restore the lobby. */
  onBackToLobby() {
    if (this.replayLaunch || this.labLaunch || this.matchLaunch) {
      if (this.match) {
        this.match.destroy();
        this.match = null;
      }
      this.destroyLabShell();
      this.allowUnloadWithoutWarning = true;
      window.location.assign(new URL("/", window.location.href).toString());
      return;
    }
    if (this.inReplayPlayback) this.net.returnToLobby();
    if (this.match) {
      this.match.destroy();
      this.match = null;
    }
    this.destroyLabShell();
    this.inReplayPlayback = false;
    this.lastObservationRunId = "";
    this.statusBadge.clearMatchMetrics();
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = true;
    dom.gameScreen.classList.remove("branch-background");
    if (dom.branchScreen) this.branchStaging.hide();
    dom.lobbyScreen.hidden = false;
    if (dom.devLinks) dom.devLinks.hidden = false;
    this.lobby.resetToBrowser();
    this.lobby.show();
    this.mountLobbySettings();
    // A new match row may have just been written server-side; pull the freshest list.
    if (this.matchHistory) this.matchHistory.refresh();
    else this._mountMatchHistory();
  }

  destroyLabShell() {
    this.labPanel?.destroy();
    this.labClient?.destroy();
    this.labControlPolicy?.destroy?.();
    this.labPanel = null;
    this.labClient = null;
    this.labControlPolicy = null;
  }

  mountLobbySettings() {
    this.settings?.setContext({
      kind: "lobby",
      spectator: false,
      replay: false,
      tabs: buildSettingsTabs({
        audio: this.audio,
        hotkeyProfiles: this.hotkeyProfiles,
        game: {
          kind: "lobby",
          prediction: {
            state: () => ({
              enabled: this.predictionEnabled,
              active: false,
              available: true,
            }),
            onToggle: () => this.setPredictionEnabled(!this.predictionEnabled),
          },
          unitRanges: {
            state: () => ({
              enabled: this.unitRangesEnabled,
              available: true,
            }),
            onToggle: () => this.setUnitRangesEnabled(!this.unitRangesEnabled),
          },
        },
      }),
    });
  }

  setPredictionEnabled(enabled) {
    this.predictionEnabled = !!enabled;
    writePredictionEnabled(this.predictionEnabled);
    if (this.match && typeof this.match.setPredictionEnabled === "function") {
      this.match.setPredictionEnabled(this.predictionEnabled);
    }
    if (this.settings?.isOpen()) {
      if (this.match && typeof this.match.mountSettings === "function") {
        this.match.mountSettings({ keepOpen: true });
      } else {
        this.mountLobbySettings();
        this.settings.open({ focus: false });
      }
    }
  }

  setUnitRangesEnabled(enabled) {
    this.unitRangesEnabled = !!enabled;
    writeUnitRangesEnabled(this.unitRangesEnabled);
    applyMatchUnitRanges(this.match, this.unitRangesEnabled);
    if (this.settings?.isOpen()) {
      if (this.match && typeof this.match.mountSettings === "function") {
        this.match.mountSettings({ keepOpen: true });
      } else {
        this.mountLobbySettings();
        this.settings.open({ focus: false });
      }
    }
  }

  onBeforeUnload(ev) {
    if (!shouldWarnBeforeUnload(this)) return;
    ev.preventDefault();
    ev.returnValue = true;
    return true;
  }

  onCloseScorePanel() {
    dom.gameOver.hidden = true;
  }

  onGameOverOverlayClick(ev) {
    if (ev.target === dom.gameOver) this.onCloseScorePanel();
  }

  _mountMatchHistory() {
    const host = document.getElementById("match-history-host");
    if (!host) return;
    if (this.matchHistory) return;
    this.matchHistory = new MatchHistory(host, {
      onReplayRoom: (room) => this.joinReplayLobby(room),
    });
  }

  joinReplayLobby(room) {
    if (!this.lobby) return false;
    dom.lobbyScreen.hidden = false;
    this.lobby.show();
    return this.lobby.joinReplayLobby(room);
  }

  /**
   * Pop a transient toast (server notices, connection problems, etc.).
   * Re-arming the timer keeps the latest message visible.
   * @param {string} text
   */
  showToast(text, timeoutMs = TOAST_MS) {
    if (!text) return;
    dom.toast.textContent = text;
    dom.toast.hidden = false;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => {
      dom.toast.hidden = true;
    }, timeoutMs);
  }

  shutdownSecondsRemaining(m) {
    const deadline = Number(m?.deadlineUnixMs);
    if (Number.isFinite(deadline) && deadline > 0) {
      return Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
    }
    const fallback = Number(m?.secondsRemaining);
    return Number.isFinite(fallback) && fallback > 0 ? Math.ceil(fallback) : 0;
  }

  formatDuration(seconds) {
    const total = Math.max(0, Math.ceil(seconds));
    const minutes = Math.floor(total / 60);
    const rem = total % 60;
    if (minutes <= 0) return `${rem}s`;
    if (rem === 0) return `${minutes}m`;
    return `${minutes}m ${rem}s`;
  }

  /**
   * Surface server connection failures in both the global toast and the lobby
   * status line, so the warning is visible before a match starts.
   * @param {string} [text]
   */
  showConnectionWarning(
    text = "Unable to connect to the server. Make sure it is running, then refresh.",
  ) {
    this.showToast(text);
    if (this.lobby) this.lobby.setStatus(text, true);
    this.labCatalog?.setStatus(text, { error: true });
  }

  /** Fetch and display the build version in the shared top-left badge. */
  async loadVersion() {
    try {
      const res = await fetch("/version", { cache: "no-store" });
      if (!res.ok) throw new Error(`version request failed: ${res.status}`);
      const text = await res.text();
      const version = text.trim() || "unknown";
      globalThis.__RTS_BUILD__ = version;
      this.statusBadge.setVersion(version);
    } catch {
      globalThis.__RTS_BUILD__ = "unknown";
      this.statusBadge.setVersion("unknown");
    }
  }
}
