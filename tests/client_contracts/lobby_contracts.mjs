// tests/client_contracts/lobby_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import fs from "node:fs";
import {
  assert,
  assertDeepEqual,
} from "./assertions.mjs";
import {
  fakeClassList,
  findFakes,
  withFakeDocument,
} from "./fakes.mjs";
import {
  MAX_LOBBY_TEAMS,
  Lobby,
  PLAYABLE_FACTIONS,
  betaFactionSelectEnabledForLocation,
  countdownSoundId,
  shouldAcceptSpectatorDrop,
  shouldAcceptTeamDrop,
  teamSlotsForLobby,
} from "../../client/src/lobby.js";
import {
  LOBBY_BROWSER_POLL_MS,
  LobbyBrowserView,
  LobbyCreateModal,
  formatLobbyAge,
  lobbyJoinIntent,
  lobbyActionLabel,
  lobbyStatusLabel,
  sortLobbySummaries,
  suggestLobbyName,
  validateLobbyName,
} from "../../client/src/lobby_browser_view.js";
import { AI_PROFILES, LobbyRosterView } from "../../client/src/lobby_view.js";

import { textWithin } from "./dom_text.mjs";

// ---------------------------------------------------------------------------
// Lobby team UI helpers
// ---------------------------------------------------------------------------
{
  assert(MAX_LOBBY_TEAMS === 4, "lobby exposes four host-managed team slots");
  assert(countdownSoundId("Drei!", 0, 3) === "countdown_drei", "countdown maps Drei to the first voice cue");
  assert(countdownSoundId("Zwei!", 1, 3) === "countdown_zwei", "countdown maps Zwei to the second voice cue");
  assert(countdownSoundId("Eins!", 2, 3) === "countdown_eins", "countdown maps Eins to the final voice cue");
  assert(countdownSoundId("2", 1, 3) === "countdown_zwei", "countdown maps numeric labels to voice cues");
  assert(countdownSoundId("Ready", 0, 3) === "countdown_drei", "three-word countdowns fall back to display order");
  assert(countdownSoundId("Go", 0, 1) === null, "non-countdown words stay silent");
  assert(
    PLAYABLE_FACTIONS.find((entry) => entry.id === "ekat")?.label === "Ekat",
    "lobby faction selector labels the ekat faction as Ekat",
  );
  assertDeepEqual(
    AI_PROFILES,
    [
      { id: "ai_2_1", label: "AI 2.1" },
      { id: "ai_turtle", label: "AI Turtle" },
    ],
    "lobby AI profile selector exposes the two supported profiles",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "rts-0-zvorygin-beta.fly.dev", pathname: "/" }),
    "lobby faction select shows on beta host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "localhost", pathname: "/" }),
    "lobby faction select shows on local runserver host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "127.0.0.1", pathname: "/" }),
    "lobby faction select shows on loopback host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "0.0.0.0", pathname: "/" }),
    "lobby faction select shows on wildcard bind host",
  );
  assert(
    !betaFactionSelectEnabledForLocation({ hostname: "rts-0-zvorygin.fly.dev", pathname: "/" }),
    "lobby faction select stays hidden on mainline host",
  );
  const slots = teamSlotsForLobby([
    { id: 3, teamId: 1 },
    { id: 4, teamId: 2 },
    { id: 9, teamId: 0, isSpectator: true },
  ]);
  assert(
    slots.length === 3 && slots[0].id === 1 && slots[1].id === 2 && slots[2].id === 3 && slots[2].isNew,
    "lobby renders occupied teams plus the first empty new-team slot",
  );
  const fullSlots = teamSlotsForLobby([
    { id: 1, teamId: 1 },
    { id: 2, teamId: 2 },
    { id: 3, teamId: 3 },
    { id: 4, teamId: 4 },
  ]);
  assert(fullSlots.length === 4 && fullSlots.every((slot) => !slot.isNew),
    "lobby omits the new-team slot when all four teams are occupied");
  const fullDuelSlots = teamSlotsForLobby([
    { id: 1, teamId: 1 },
    { id: 2, teamId: 1 },
  ], 2);
  assert(fullDuelSlots.length === 2 && fullDuelSlots[0].id === 1 && fullDuelSlots[1].id === 2 && fullDuelSlots[1].isNew,
    "full capped lobbies keep one empty team target for active-player reassignment");
  const splitDuelSlots = teamSlotsForLobby([
    { id: 1, teamId: 1 },
    { id: 2, teamId: 2 },
  ], 2);
  assert(splitDuelSlots.length === 2 && splitDuelSlots.every((slot) => !slot.isNew),
    "full capped lobbies omit extra empty team targets once occupied teams reach the cap");
  assert(
    shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 2, teamId: 2 },
      isHost: true,
      countdownActive: false,
    }),
    "host can drag an active human player to spectators",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 9, teamId: 2, isAi: true },
      isHost: true,
      countdownActive: false,
    }),
    "spectator drop rejects AI seats",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: true,
      countdownActive: false,
    }),
    "spectator drop rejects existing spectators",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 2, teamId: 2 },
      isHost: false,
      countdownActive: false,
    }),
    "spectator drop is host-only",
  );
  assert(
    shouldAcceptTeamDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: true,
      countdownActive: false,
    }),
    "host can drag a spectator back into a team slot",
  );
  assert(
    !shouldAcceptTeamDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: true,
      countdownActive: false,
      playerCount: 2,
      maxPlayers: 2,
    }),
    "team drop rejects spectator return when the selected map is full",
  );
  assert(
    !shouldAcceptTeamDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: false,
      countdownActive: false,
    }),
    "team drop is host-only",
  );
}

{
  withFakeDocument(() => {
    const root = document.createElement("div");
    const view = new LobbyRosterView(root);
    let selectedProfile = null;
    view.render({
      players: [
        { id: 1, name: "Host", color: "#0072b2", ready: false, teamId: 1 },
        {
          id: 2,
          name: "AI 2.1",
          color: "#d55e00",
          ready: true,
          teamId: 2,
          isAi: true,
          aiProfileId: "ai_2_1",
        },
      ],
      myId: 1,
      hostId: 1,
      isHost: true,
      countdownActive: false,
      playerCount: 2,
      maxPlayers: 4,
      onSetAiProfile: (id, aiProfileId) => {
        selectedProfile = { id, aiProfileId };
      },
    });

    const profileSelectors = findFakes(
      root,
      (el) => el.tagName === "SELECT" && el.className === "player-ai-profile-select",
    );
    assert(
      profileSelectors.length === 1 && profileSelectors[0].value === "ai_2_1",
      "host lobby exposes an AI 2.1 profile selector",
    );
    assert(textWithin(root).includes("AI 2.1"), "host lobby labels AI seats as AI 2.1");
    profileSelectors[0].value = "ai_turtle";
    profileSelectors[0].listeners.change?.();
    assertDeepEqual(
      selectedProfile,
      { id: 2, aiProfileId: "ai_turtle" },
      "host lobby sends a selected canonical AI profile",
    );

    const turtleRoot = document.createElement("div");
    const turtleView = new LobbyRosterView(turtleRoot);
    turtleView.render({
      players: [
        {
          id: 2,
          name: "AI Turtle",
          color: "#d55e00",
          ready: true,
          teamId: 2,
          isAi: true,
          aiProfileId: "ai_turtle",
        },
      ],
      myId: 1,
      hostId: 1,
      isHost: true,
      countdownActive: false,
      playerCount: 1,
      maxPlayers: 4,
    });

    assert(
      textWithin(turtleRoot).includes("AI Turtle"),
      "host lobby labels Turtle AI seats as AI Turtle",
    );
  });
}

{
  let stoppedPolling = 0;
  let startedPolling = 0;
  let clearedCountdown = 0;
  let hidReplayPrompt = 0;
  let renderedBrowser = null;
  let statusText = "old";
  const lobby = Object.assign(Object.create(Lobby.prototype), {
    root: { hidden: false, classList: fakeClassList() },
    net: { playerId: 7 },
    roomBlock: { hidden: false },
    elSetupKicker: { textContent: "Host controls" },
    elSetupTitle: { textContent: "Match setup" },
    elPlayers: { innerHTML: "occupied" },
    elRoomDisplay: { textContent: "Old room" },
    elMapSummary: { textContent: "No Terrain", hidden: true },
    elSeatsSummary: { textContent: "2 / 4" },
    elObserversSummary: { textContent: "1" },
    btnJoin: { textContent: "Switch room" },
    btnCreateLobby: { hidden: true, disabled: true },
    btnReady: {
      textContent: "Unready",
      disabled: false,
      classList: fakeClassList(),
      setAttribute(name, value) {
        this[name] = String(value);
      },
    },
    btnStart: { disabled: false, classList: fakeClassList() },
    selMap: null,
    browserView: { rows: [{ room: "Fresh lobby" }] },
    _joined: true,
    _ready: true,
    _spectator: false,
    _hostId: 7,
    _canStart: true,
    _teamPreset: "custom",
    _selectedMap: "No Terrain",
    _availableMaps: [{ name: "No Terrain" }],
    _playerCount: 2,
    _browserActionPending: true,
    _browserConnected: true,
    _fetchImpl: () => Promise.resolve(),
    _pendingBrowserJoinRoom: "Old room",
    _pendingReplayRoom: "Old room",
    _promptReturnFocus: {},
    _stopLobbyBrowserPolling() {
      stoppedPolling += 1;
    },
    _startLobbyBrowserPolling() {
      startedPolling += 1;
    },
    _clearCountdown() {
      clearedCountdown += 1;
    },
    _hideReplayPrompt() {
      hidReplayPrompt += 1;
    },
    _renderLobbyBrowser(args) {
      renderedBrowser = args;
    },
    _reflectTeamPreset() {},
    setStatus(text) {
      statusText = text;
    },
  });

  lobby.resetToBrowser();

  assert(!lobby._joined && !lobby._ready && !lobby._spectator, "lobby reset clears joined/ready/spectator state");
  assert(lobby._hostId === null && lobby._canStart === false, "lobby reset clears host/start state");
  assert(lobby._pendingBrowserJoinRoom === "" && lobby._browserActionPending === false,
    "lobby reset clears pending browser join state");
  assert(lobby._pendingReplayRoom === "" && lobby._promptReturnFocus === null,
    "lobby reset clears replay prompt state");
  assert(lobby.elPlayers.innerHTML === "", "lobby reset clears the stale roster");
  assert(lobby.roomBlock.hidden, "lobby reset hides the joined-room setup panel");
  assert(lobby.elSetupTitle.textContent === "Lobby browser", "lobby reset restores browser title");
  assert(lobby.btnCreateLobby.hidden === false && lobby.btnCreateLobby.disabled === false,
    "lobby reset re-enables create-lobby affordance");
  assert(lobby.btnReady.textContent === "Ready" && lobby.btnReady["aria-pressed"] === "false",
    "lobby reset restores the ready button to inactive state");
  assert(lobby.btnStart.disabled, "lobby reset disables stale start control");
  assert(lobby.elSeatsSummary.textContent === "0 / 4" && lobby.elObserversSummary.textContent === "0",
    "lobby reset clears stale room summary counts");
  assert(stoppedPolling === 1 && startedPolling === 1 && clearedCountdown === 1 && hidReplayPrompt === 1,
    "lobby reset restarts browser polling and clears transient room UI");
  assert(renderedBrowser?.error === "" && statusText === "", "lobby reset redraws browser without stale errors");
}

{
  let rosterArgs = null;
  let stoppedPolling = 0;
  let statusText = "old";
  const root = { hidden: false, classList: fakeClassList() };
  const seatsCell = { hidden: false };
  const lobby = Object.assign(Object.create(Lobby.prototype), {
    root,
    net: { playerId: 7 },
    roomBlock: { hidden: true },
    elSetupKicker: { textContent: "" },
    elSetupTitle: { textContent: "" },
    elPlayers: { innerHTML: "" },
    elRoomDisplay: { textContent: "" },
    elMapSummary: { textContent: "", hidden: true },
    elSeatsSummary: { textContent: "" },
    elSeatsSummaryCell: seatsCell,
    elObserversSummary: { textContent: "" },
    btnReady: {
      hidden: false,
      textContent: "",
      disabled: false,
      classList: fakeClassList(),
      setAttribute(name, value) {
        this[name] = String(value);
      },
    },
    btnStart: { disabled: true, textContent: "", classList: fakeClassList() },
    selMap: { disabled: false, hidden: false, options: [], value: "" },
    rosterView: {
      render(args) {
        rosterArgs = args;
      },
    },
    browserView: null,
    _joined: false,
    _ready: false,
    _spectator: false,
    _countdownActive: false,
    _browserActionPending: true,
    _pendingBrowserJoinRoom: "old",
    _stopLobbyBrowserPolling() {
      stoppedPolling += 1;
    },
    _reflectTeamPreset() {},
    _reflectCreateButton() {},
    _betaFactionSelectEnabled() {
      return false;
    },
    setStatus(text) {
      statusText = text;
    },
  });

  lobby._renderLobby({
    room: "__match_replay__:00000001",
    kind: "replay",
    hostId: 7,
    players: [
      { id: 7, name: "Replay Host", isSpectator: true },
      { id: 8, name: "Hidden Active Seat", isSpectator: false },
    ],
    canStart: true,
    teamPreset: "custom",
    map: "Lowlands",
    maps: [],
  });

  assert(root.classList.contains("is-replay-lobby"), "joined replay lobbies set a replay UI state class");
  assert(!lobby.roomBlock.hidden, "joined replay lobbies show the joined-room block");
  assert(lobby.elSetupKicker.textContent === "Group replay" && lobby.elSetupTitle.textContent === "Replay lobby",
    "joined replay lobbies switch the setup panel copy");
  assert(lobby.btnReady.hidden, "joined replay lobbies hide the Ready button");
  assert(!lobby.btnStart.disabled && lobby.btnStart.textContent === "Start replay",
    "replay lobby hosts can start whenever the server says canStart");
  assert(lobby.selMap.hidden && lobby.selMap.disabled, "joined replay lobbies hide map selection");
  assert(seatsCell.hidden && lobby.elSeatsSummary.textContent === "",
    "joined replay lobbies hide active-seat counts");
  assert(lobby.elObserversSummary.textContent === "1", "joined replay lobbies count spectator occupants only");
  assert(rosterArgs?.spectatorOnly === true, "joined replay lobbies render the roster in spectator-only mode");
  assert(stoppedPolling === 1 && statusText === "", "joined replay lobby render clears pending browser state");
}

{
  let rosterArgs = null;
  let stoppedPolling = 0;
  const root = { hidden: false, classList: fakeClassList() };
  const lobby = Object.assign(Object.create(Lobby.prototype), {
    root,
    net: { playerId: 7 },
    roomBlock: { hidden: true },
    elSetupKicker: { textContent: "" },
    elSetupTitle: { textContent: "" },
    elPlayers: { innerHTML: "" },
    elRoomDisplay: { textContent: "" },
    elMapSummary: { textContent: "", hidden: true },
    elSeatsSummary: { textContent: "" },
    elSeatsSummaryCell: { hidden: false },
    elObserversSummary: { textContent: "" },
    btnReady: {
      hidden: false,
      textContent: "",
      disabled: false,
      classList: fakeClassList(),
      setAttribute(name, value) {
        this[name] = String(value);
      },
    },
    btnStart: { disabled: true, textContent: "", classList: fakeClassList() },
    selMap: null,
    rosterView: {
      render(args) {
        rosterArgs = args;
      },
    },
    browserView: null,
    _joined: false,
    _ready: false,
    _spectator: false,
    _countdownActive: false,
    _browserActionPending: false,
    _pendingBrowserJoinRoom: "",
    _stopLobbyBrowserPolling() {
      stoppedPolling += 1;
    },
    _reflectTeamPreset() {},
    _reflectCreateButton() {},
    _betaFactionSelectEnabled() {
      return false;
    },
    setStatus() {},
  });

  lobby._renderLobby({
    room: "duel",
    kind: "normal",
    hostId: 7,
    players: [
      { id: 7, name: "Host", color: "#0072b2", teamId: 1, ready: true, isSpectator: false },
      { id: 8, name: "Guest", color: "#d55e00", teamId: 2, ready: true, isSpectator: false },
    ],
    canStart: true,
    teamPreset: "custom",
    map: "1v1 No Terrain",
    maps: [
      {
        name: "1v1 No Terrain",
        description: "Two-player no-terrain map scaffold for 1v1 terrain editing.",
        minPlayers: 1,
        maxPlayers: 2,
      },
    ],
  });

  assert(lobby.elSeatsSummary.textContent === "2 / 2",
    "joined lobby summary uses selected map capacity");
  assert(rosterArgs?.maxPlayers === 2, "lobby roster receives selected map capacity");
  assert(stoppedPolling === 1, "normal lobby render clears pending browser polling");
}

// ---------------------------------------------------------------------------
// Lobby browser UI helpers
// ---------------------------------------------------------------------------
{
  const now = 200_000_000;
  assert(LOBBY_BROWSER_POLL_MS === 1500, "lobby browser polls inside the 1-2 second contract");
  assert(formatLobbyAge(now - 5_000, now) === "just now", "lobby browser formats fresh ages");
  assert(formatLobbyAge(now - 3 * 60_000, now) === "3m ago", "lobby browser formats minute ages");
  assert(formatLobbyAge(now - 2 * 60 * 60_000, now) === "2h ago", "lobby browser formats hour ages");
  assert(lobbyStatusLabel("fullSpectatorOnly") === "Full", "full lobby rows get a distinct status label");
  assert(lobbyActionLabel("fullSpectatorOnly") === "Join as spectator",
    "full lobby rows advertise spectator joining");
  assert(lobbyActionLabel("inGame") === "Spectate",
    "in-progress lobby rows advertise live spectating");
  assert(lobbyStatusLabel({ joinState: "fullSpectatorOnly", kind: "replay" }) === "Replay",
    "replay lobby rows get a replay status label");
  assert(lobbyActionLabel({ joinState: "fullSpectatorOnly", kind: "replay" }) === "Join replay",
    "replay lobby rows advertise replay joining");
  assertDeepEqual(lobbyJoinIntent({ joinState: "open" }), { state: "open", joinable: true, spectator: false },
    "open lobby rows join as active players");
  assertDeepEqual(lobbyJoinIntent({ joinState: "fullSpectatorOnly" }),
    { state: "fullSpectatorOnly", joinable: true, spectator: true },
    "full waiting lobby rows join as spectators");
  assertDeepEqual(lobbyJoinIntent({ joinState: "inGame" }),
    { state: "inGame", joinable: true, spectator: true },
    "in-progress lobby rows join as spectators");
  assertDeepEqual(lobbyJoinIntent({ joinState: "fullSpectatorOnly", kind: "replay" }),
    { state: "fullSpectatorOnly", joinable: true, spectator: true },
    "replay lobby rows always join as spectators");
  assert(validateLobbyName(" Alpha ").ok, "lobby create accepts trimmed plain names");
  assert(!validateLobbyName("   ").ok, "lobby create rejects empty names");
  assert(!validateLobbyName("__lab__:sandbox").ok, "lobby create rejects reserved internal prefixes");
  assert(!validateLobbyName("x".repeat(65)).ok, "lobby create mirrors the server byte-length cap");
  assert(suggestLobbyName("Alex") === "Alex's lobby", "lobby create suggests a lobby from player name");
  assert(suggestLobbyName("") === "Commander's lobby", "lobby create suggestion falls back when player name is blank");
  assert(validateLobbyName(suggestLobbyName("x".repeat(120))).ok,
    "lobby create suggestion stays within the public lobby name limit");
  assert(validateLobbyName(suggestLobbyName("__lab__:sandbox")).ok,
    "lobby create suggestion avoids reserved internal prefixes");
  const indexHtml = fs.readFileSync(new URL("../../client/index.html", import.meta.url), "utf8");
  assert(indexHtml.includes('class="lobby-manual-room" hidden'),
    "manual room-name join controls stay outside the normal pre-join product path");
  assert(indexHtml.includes("#lobby-room and #lobby-join remain hidden compatibility controls"),
    "DOM contract documents room-name controls as hidden compatibility only");
  assert(indexHtml.includes('id="lobby-lab-open"'),
    "normal lobby exposes a direct lab entry affordance");
  assert(indexHtml.includes('href="/lab"'),
    "normal lobby lab entry opens the default lab scenario without URL overrides");
  assert(!indexHtml.includes('id="lobby-quickstart"'),
    "normal lobby does not render the legacy quickstart control");
  assert(!indexHtml.includes("Debug mode"),
    "normal lobby copy no longer advertises Debug mode as the experimentation path");
  const staticStepButton = indexHtml.match(/<button[^>]*class="[^"]*\broom-time-step-btn\b[^"]*"[^>]*>/)?.[0] || "";
  assert(staticStepButton.includes("data-step-room-time"),
    "static dev scenario Step button uses the neutral room-time step contract");
  assert(!staticStepButton.includes("data-step-dev-tick"),
    "static dev scenario Step button does not use stale dev-specific step markup");

  const sorted = sortLobbySummaries([
    { room: "old-open", hostName: "A", createdAtUnixMs: 100, joinState: "open" },
    { room: "in-game", hostName: "B", createdAtUnixMs: 900, joinState: "inGame" },
    { room: "full", hostName: "C", createdAtUnixMs: 800, joinState: "fullSpectatorOnly" },
    { room: "new-open", hostName: "D", createdAtUnixMs: 700, joinState: "open" },
    { room: "starting", hostName: "E", createdAtUnixMs: 950, joinState: "starting" },
  ]);
  assertDeepEqual(
    sorted.map((row) => row.room),
    ["new-open", "old-open", "full", "in-game", "starting"],
    "lobby browser sorts open, full, in-game, and starting rows by joinability then age",
  );

  withFakeDocument(() => {
    const joins = [];
    let createClicks = 0;
    const rowsRoot = {
      children: [],
      replaceChildren(...children) {
        this.children = children;
      },
    };
    const statusEl = { textContent: "" };
    const root = {
      classList: fakeClassList(),
      querySelector(selector) {
        if (selector === "#lobby-browser-rows") return rowsRoot;
        if (selector === "#lobby-browser-status") return statusEl;
        return null;
      },
    };
    const view = new LobbyBrowserView(root);
    view.render({
      rows: [],
      nowMs: now,
      onCreateLobby: () => { createClicks += 1; },
      onJoinLobby: (row, options) => joins.push({ room: row.room, spectator: !!options?.spectator }),
    });
    assert(textWithin(rowsRoot).includes("No lobbies"), "lobby browser renders compact empty state");
    findFakes(rowsRoot, (el) => el.tagName === "BUTTON" && el.textContent === "Create lobby")[0]?.click();
    assert(createClicks === 1, "empty lobby browser create action opens the create flow");
    view.render({
      rows: [
        {
          room: "Open Lobby",
          hostName: "Host A",
          map: "Default",
          createdAtUnixMs: now - 30_000,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "open",
        },
        {
          room: "Alpha Long Lobby",
          hostName: "Host",
          map: "No Terrain",
          createdAtUnixMs: now - 60_000,
          occupiedSlots: 4,
          maxSlots: 4,
          spectatorCount: 1,
          joinState: "fullSpectatorOnly",
        },
        {
          room: "Replay Room",
          kind: "replay",
          hostName: "Archivist",
          map: "Lowlands",
          createdAtUnixMs: now - 20_000,
          occupiedSlots: 0,
          maxSlots: 0,
          spectatorCount: 2,
          joinState: "fullSpectatorOnly",
        },
        {
          room: "Locked Match",
          hostName: "Host C",
          map: "Default",
          createdAtUnixMs: now - 90_000,
          occupiedSlots: 4,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "inGame",
        },
        {
          room: "Countdown Match",
          hostName: "Host D",
          map: "Default",
          createdAtUnixMs: now - 10_000,
          occupiedSlots: 2,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "starting",
        },
        {
          room: "Unknown State",
          hostName: "Host E",
          map: "Default",
          createdAtUnixMs: now - 20_000,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "mystery",
        },
      ],
      nowMs: now,
    });
    assert(textWithin(rowsRoot).includes("Alpha Long Lobby"), "lobby browser renders lobby names");
    assert(textWithin(rowsRoot).includes("No Terrain"), "lobby browser renders map names");
    assert(textWithin(rowsRoot).includes("4 / 4 +1 obs"), "lobby browser renders active slots and spectators");
    assert(textWithin(rowsRoot).includes("2 spectators"), "lobby browser renders replay spectator counts");
    assert(textWithin(rowsRoot).includes("Join as spectator"), "lobby browser renders row action state");
    assert(textWithin(rowsRoot).includes("Join replay"), "lobby browser renders replay row actions");
    const row = rowsRoot.children.find((child) => child.dataset.joinState === "fullSpectatorOnly");
    assert(row.dataset.joinState === "fullSpectatorOnly", "lobby browser pins row join-state data");
    const replayRow = rowsRoot.children.find((child) => child.dataset.kind === "replay");
    assert(replayRow?.dataset.kind === "replay", "lobby browser pins replay row kind data");
    const buttons = findFakes(rowsRoot, (el) => el.tagName === "BUTTON");
    const openButton = buttons.find((button) => button.textContent === "Join lobby");
    const spectatorButton = buttons.find((button) => button.textContent === "Join as spectator");
    const replayButton = buttons.find((button) => button.textContent === "Join replay");
    const inGameButton = buttons.find((button) => button.textContent === "Spectate");
    const startingButton = buttons.find((button) => button.textContent === "Starting");
    const staleButton = buttons.find((button) => button.textContent === "Stale");
    assert(!openButton?.disabled, "open lobby row action is enabled");
    assert(!spectatorButton?.disabled, "full lobby row action joins as spectator");
    assert(!replayButton?.disabled, "replay lobby row action joins as spectator");
    assert(!inGameButton?.disabled, "in-game lobby row action joins as spectator");
    assert(startingButton?.disabled, "countdown lobby row action stays disabled");
    assert(staleButton?.disabled, "unknown lobby row action stays disabled as stale");
    openButton.click();
    spectatorButton.click();
    replayButton.click();
    inGameButton.click();
    assertDeepEqual(joins, [
      { room: "Open Lobby", spectator: false },
      { room: "Alpha Long Lobby", spectator: true },
      { room: "Replay Room", spectator: true },
      { room: "Locked Match", spectator: true },
    ], "lobby browser row actions carry active vs spectator join intent");
    view.render({
      rows: [
        {
          room: "Refresh Failed",
          hostName: "Host F",
          map: "Default",
          createdAtUnixMs: now,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "open",
        },
      ],
      error: "Lobby list unavailable.",
    });
    const disabledAfterError = findFakes(rowsRoot,
      (el) => el.tagName === "BUTTON" && el.textContent === "Join lobby")[0];
    assert(disabledAfterError?.disabled, "failed lobby-list refresh disables stale row actions");
  });

  withFakeDocument(() => {
    const root = document.createElement("div");
    const view = new LobbyRosterView(root);
    view.render({
      players: [
        { id: 7, name: "Replay Host", color: "#7aa0b5", isSpectator: true },
        { id: 8, name: "Hidden Active Seat", color: "#b57a7a", isSpectator: false },
      ],
      myId: 7,
      hostId: 7,
      isHost: true,
      countdownActive: false,
      spectatorOnly: true,
    });
    const text = textWithin(root);
    assert(text.includes("Replay lobby") && text.includes("1 viewer"),
      "replay lobby roster renders a replay spectator-only section");
    assert(text.includes("Replay Host") && !text.includes("Hidden Active Seat"),
      "replay lobby roster renders only spectator occupants");
    assert(!text.includes("Add AI") && !text.includes("Human player"),
      "replay lobby roster omits AI and active-seat controls");
  });

  await withFakeDocument(async () => {
    const host = document.createElement("section");
    const trigger = document.createElement("button");
    let submitted = "";
    const modal = new LobbyCreateModal(host, {
      onSubmit: async (room) => {
        submitted = room;
        modal.setError("Lobby name is already in use.");
        return false;
      },
    });
    modal.open(trigger, { initialValue: "Alex's lobby" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    const input = findFakes(host, (el) => el.tagName === "INPUT")[0];
    const submit = findFakes(host, (el) => el.tagName === "BUTTON" && el.textContent === "Create lobby")[0];
    assert(document.activeElement === input, "create lobby modal moves focus to the name input");
    assert(input.value === "Alex's lobby", "create lobby modal prepopulates the suggested lobby name");
    assert(!submit.disabled, "create lobby modal enables submit when the suggested lobby name is valid");
    input.value = "taken";
    input.listeners.input?.({ target: input });
    assert(!submit.disabled, "create lobby modal enables submit for a valid name");
    submit.click();
    await Promise.resolve();
    assert(submitted === "taken", "create lobby modal submits the trimmed lobby name");
    assert(textWithin(host).includes("Lobby name is already in use."),
      "duplicate create failures are displayed inline");
    modal.close();
    assert(document.activeElement === trigger, "create lobby modal returns focus to the trigger");
    modal.destroy();
  });
}

// ---------------------------------------------------------------------------
