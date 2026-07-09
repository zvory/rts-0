// Lobby view helpers: pure DOM rendering for pre-match teams, seats, and observers.
// The Lobby controller owns networking/state; this module owns structure.

export const MAX_LOBBY_TEAMS = 4;
export const AI_PROFILES = Object.freeze([
  { id: "ai_1_0", label: "AI 1.0" },
  { id: "ai_1_1", label: "AI 1.1" },
  { id: "ai_1_2", label: "AI 1.2" },
  { id: "ai_2_0", label: "AI 2.0" },
]);

const AI_PROFILE_ALIASES = Object.freeze({
  ai_1_0_tech: "ai_1_0",
  ai_1_1_tank_mg: "ai_1_1",
  ai_1_2_wave_cohorts: "ai_1_2",
  ai_2_0_tank_pressure: "ai_2_0",
});

const STABLE_DEFAULT_AI_PROFILE_ID = "ai_1_2";
export const DEFAULT_AI_PROFILE_ID =
  AI_PROFILES.some((entry) => entry.id === STABLE_DEFAULT_AI_PROFILE_ID)
    ? STABLE_DEFAULT_AI_PROFILE_ID
    : AI_PROFILES[0].id;

export function teamSlotsForLobby(players = [], maxPlayers = MAX_LOBBY_TEAMS) {
  const slotLimit = lobbyTeamSlotLimit(maxPlayers);
  const seatedPlayers = players.filter((player) => !player.isSpectator);
  const occupied = [];
  for (let teamId = 1; teamId <= MAX_LOBBY_TEAMS; teamId += 1) {
    if (seatedPlayers.some((player) => Number(player.teamId) === teamId)) {
      occupied.push({ id: teamId, isNew: false });
    }
  }
  if (occupied.length < slotLimit) {
    const emptyId = Array.from({ length: MAX_LOBBY_TEAMS }, (_, idx) => idx + 1)
      .find((teamId) => !occupied.some((slot) => slot.id === teamId));
    if (emptyId != null) occupied.push({ id: emptyId, isNew: true });
  }
  return occupied;
}

function lobbyTeamSlotLimit(maxPlayers) {
  const value = Number(maxPlayers);
  if (!Number.isFinite(value)) return MAX_LOBBY_TEAMS;
  return Math.min(MAX_LOBBY_TEAMS, Math.max(1, Math.trunc(value)));
}

export function splitLobbyPlayers(players = []) {
  return {
    seatedPlayers: players.filter((player) => !player.isSpectator),
    spectatorPlayers: players.filter((player) => player.isSpectator),
  };
}

export function shouldAcceptSpectatorDrop({
  draggedPlayer,
  isHost,
  countdownActive,
} = {}) {
  return (
    !!isHost &&
    !countdownActive &&
    !!draggedPlayer &&
    !draggedPlayer.isAi &&
    !draggedPlayer.isSpectator
  );
}

export function shouldAcceptTeamDrop({
  draggedPlayer,
  isHost,
  countdownActive,
  playerCount,
  maxPlayers,
} = {}) {
  if (!isHost || countdownActive || !draggedPlayer) return false;
  if (
    draggedPlayer.isSpectator &&
    Number.isFinite(Number(playerCount)) &&
    Number.isFinite(Number(maxPlayers)) &&
    Number(playerCount) >= Number(maxPlayers)
  ) {
    return false;
  }
  return true;
}

export class LobbyRosterView {
  constructor(rootEl) {
    this.root = rootEl;
  }

  render({
    players,
    myId,
    hostId,
    isHost,
    countdownActive,
    spectatorOnly = false,
    playerCount,
    maxPlayers,
    betaFactionSelect,
    onAddAi,
    onRemoveAi,
    onSetTeam,
    onSetSpectator,
    onSetFaction,
    onSetAiProfile,
  }) {
    if (!this.root) return;
    this.root.innerHTML = "";
    this.root.classList?.toggle("is-spectator-only", !!spectatorOnly);

    const { seatedPlayers, spectatorPlayers } = splitLobbyPlayers(players);
    if (!spectatorOnly) {
      const slots = teamSlotsForLobby(seatedPlayers, maxPlayers);
      const occupiedSlotCount = slots.filter((slot) => !slot.isNew).length;
      for (const slot of slots) {
        const teamPlayers = seatedPlayers.filter((player) => Number(player.teamId) === Number(slot.id));
        this.root.appendChild(this._buildTeamColumn({
          slot,
          occupiedSlotCount,
          players: teamPlayers,
          allPlayers: players,
          myId,
          hostId,
          isHost,
          countdownActive,
          playerCount,
          maxPlayers,
          betaFactionSelect,
          onAddAi,
          onRemoveAi,
          onSetTeam,
          onSetSpectator,
          onSetFaction,
          onSetAiProfile,
        }));
      }
    }

    this.root.appendChild(this._buildSpectatorSection({
      players: spectatorPlayers,
      allPlayers: players,
      myId,
      hostId,
      isHost,
      countdownActive,
      spectatorOnly,
      onSetSpectator,
    }));
  }

  _buildTeamColumn({
    slot,
    occupiedSlotCount,
    players,
    allPlayers,
    myId,
    hostId,
    isHost,
    countdownActive,
    playerCount,
    maxPlayers,
    betaFactionSelect,
    onAddAi,
    onRemoveAi,
    onSetTeam,
    onSetSpectator,
    onSetFaction,
    onSetAiProfile,
  }) {
    const section = document.createElement("section");
    section.className = "lobby-team-card team-row";
    if (slot.isNew) section.classList.add("is-new-team");
    section.setAttribute("aria-label", `Team ${slot.id}`);
    if (isHost && !countdownActive) {
      section.addEventListener("dragover", (ev) => {
        ev.preventDefault();
        section.classList.add("is-drop-target");
      });
      section.addEventListener("dragleave", () => section.classList.remove("is-drop-target"));
      section.addEventListener("drop", (ev) => {
        ev.preventDefault();
        section.classList.remove("is-drop-target");
        const draggedId = Number(ev.dataTransfer?.getData("application/x-rts-player-id"));
        if (!draggedId) return;
        const dragged = allPlayers.find((player) => player.id === draggedId);
        if (!shouldAcceptTeamDrop({
          draggedPlayer: dragged,
          isHost,
          countdownActive,
          playerCount,
          maxPlayers,
        })) return;
        if (dragged && Number(dragged.teamId) === Number(slot.id)) return;
        if (dragged?.isSpectator) onSetSpectator?.(draggedId, false);
        onSetTeam?.(draggedId, Number(slot.id));
      });
    }

    const header = document.createElement("header");
    header.className = "lobby-team-header";

    const title = document.createElement("div");
    title.className = "lobby-team-title";
    const kicker = document.createElement("span");
    kicker.className = "lobby-kicker";
    kicker.textContent = slot.isNew ? "Open slot" : "";
    const name = document.createElement("h2");
    name.textContent = slot.isNew ? "New team" : occupiedSlotCount === 1 ? "Team" : `Team ${slot.id}`;
    if (kicker.textContent) title.appendChild(kicker);
    title.appendChild(name);

    const count = document.createElement("span");
    count.className = "lobby-team-count team-row-count";
    count.textContent = String(players.length);

    header.append(title, count);
    if (isHost && slot.isNew) {
      const add = document.createElement("button");
      add.type = "button";
      add.className = "team-add-ai btn";
      add.textContent = "Add AI";
      add.title = "Add AI to a new team";
      add.disabled = countdownActive || playerCount >= maxPlayers;
      add.addEventListener("click", () => {
        if (!add.disabled) onAddAi?.(slot.id);
      });
      header.appendChild(add);
    }

    const seats = document.createElement("div");
    seats.className = "lobby-seat-list";
    for (const player of players) {
      seats.appendChild(this._buildSeatRow({
        player,
        myId,
        hostId,
        isHost,
        countdownActive,
        betaFactionSelect,
        onRemoveAi,
        onSetFaction,
        onSetAiProfile,
      }));
    }
    if (players.length === 0) {
      const empty = document.createElement("div");
      empty.className = "lobby-empty-team";
      empty.textContent = isHost ? "Drop a player here" : "Waiting for assignment";
      seats.appendChild(empty);
    }

    section.append(header, seats);
    return section;
  }

  _buildSeatRow({
    player,
    myId,
    hostId,
    isHost,
    countdownActive,
    betaFactionSelect,
    onRemoveAi,
    onSetFaction,
    onSetAiProfile,
  }) {
    const row = document.createElement("div");
    row.className = "player-row lobby-seat";
    if (player.id === myId) row.classList.add("is-you");
    if (player.isAi) row.classList.add("is-ai");
    if (isHost && !countdownActive) {
      row.draggable = true;
      row.addEventListener("dragstart", (ev) => {
        ev.dataTransfer?.setData("application/x-rts-player-id", String(player.id));
        ev.dataTransfer.effectAllowed = "move";
        row.classList.add("is-dragging");
      });
      row.addEventListener("dragend", () => row.classList.remove("is-dragging"));
    }

    const swatch = document.createElement("span");
    swatch.className = "player-color";
    swatch.style.background = player.color || "#888";
    swatch.setAttribute("aria-hidden", "true");

    const body = document.createElement("div");
    body.className = "lobby-seat-body";

    const nameLine = document.createElement("div");
    nameLine.className = "lobby-seat-name";
    const name = document.createElement("span");
    name.className = "player-name";
    name.textContent = player.name || `Player ${player.id}`;
    nameLine.appendChild(name);

    const tags = document.createElement("span");
    tags.className = "player-tags";
    if (player.id === hostId) {
      tags.appendChild(tag("host", "Host"));
    }
    if (player.isAi) {
      tags.appendChild(tag("ai", "AI"));
    }
    nameLine.appendChild(tags);
    if (betaFactionSelect) {
      nameLine.appendChild(this._buildFactionControl({
        player,
        myId,
        countdownActive,
        onSetFaction,
      }));
    }

    const meta = document.createElement("div");
    meta.className = "lobby-seat-meta";
    if (player.isAi && isHost) {
      meta.appendChild(this._buildAiProfileControl({
        player,
        countdownActive,
        onSetAiProfile,
      }));
    } else {
      meta.textContent = player.isAi ? aiProfileLabel(player.aiProfileId) : "Human player";
    }

    body.append(nameLine, meta);

    const controls = document.createElement("div");
    controls.className = "lobby-seat-controls";
    controls.appendChild(this._buildReadyState(player, isHost, onRemoveAi));

    row.append(swatch, body, controls);
    return row;
  }

  _buildFactionControl({ player, myId, countdownActive, onSetFaction }) {
    if (player.isAi) {
      const label = document.createElement("span");
      label.className = "player-faction-label";
      label.textContent = factionLabel(player.factionId);
      return label;
    }

    const select = document.createElement("select");
    select.className = "player-faction-select";
    select.setAttribute("aria-label", `${player.name || "Player"} faction`);
    for (const entry of PLAYABLE_FACTIONS) {
      const option = document.createElement("option");
      option.value = entry.id;
      option.textContent = entry.label;
      select.appendChild(option);
    }
    select.value = playableFactionId(player.factionId);
    select.disabled = countdownActive || player.id !== myId || player.isSpectator;
    select.addEventListener("change", () => {
      if (!select.disabled) onSetFaction?.(select.value);
    });
    return select;
  }

  _buildAiProfileControl({ player, countdownActive, onSetAiProfile }) {
    const select = document.createElement("select");
    select.className = "player-ai-profile-select";
    select.setAttribute("aria-label", `${player.name || "AI"} profile`);
    for (const entry of AI_PROFILES) {
      const option = document.createElement("option");
      option.value = entry.id;
      option.textContent = entry.label;
      select.appendChild(option);
    }
    select.value = playableAiProfileId(player.aiProfileId);
    select.disabled = countdownActive;
    select.addEventListener("change", () => {
      if (!select.disabled) onSetAiProfile?.(player.id, select.value);
    });
    return select;
  }

  _buildReadyState(player, isHost, onRemoveAi) {
    if (player.isAi && isHost) {
      const remove = document.createElement("button");
      remove.className = "player-remove btn";
      remove.type = "button";
      remove.textContent = "Remove";
      remove.title = "Remove AI";
      remove.setAttribute("aria-label", `Remove ${player.name || "AI"}`);
      remove.addEventListener("click", () => onRemoveAi?.(player.id));
      return remove;
    }

    const ready = document.createElement("span");
    ready.className = "player-ready";
    if (player.isAi || player.ready) {
      ready.classList.add("ready");
      ready.textContent = "Ready";
    } else {
      ready.classList.add("waiting");
      ready.textContent = "Waiting";
    }
    return ready;
  }

  _buildSpectatorSection({
    players,
    allPlayers,
    myId,
    hostId,
    isHost,
    countdownActive,
    spectatorOnly = false,
    onSetSpectator,
  }) {
    const section = document.createElement("section");
    section.className = "lobby-spectator-card";
    if (spectatorOnly) section.classList.add("is-replay-lobby");
    section.setAttribute("aria-label", spectatorOnly ? "Replay spectators" : "Spectators");
    if (!spectatorOnly && isHost && !countdownActive) {
      section.addEventListener("dragover", (ev) => {
        ev.preventDefault();
        section.classList.add("is-drop-target");
      });
      section.addEventListener("dragleave", () => section.classList.remove("is-drop-target"));
      section.addEventListener("drop", (ev) => {
        ev.preventDefault();
        section.classList.remove("is-drop-target");
        const draggedId = Number(ev.dataTransfer?.getData("application/x-rts-player-id"));
        const draggedPlayer = allPlayers.find((player) => player.id === draggedId);
        if (!shouldAcceptSpectatorDrop({ draggedPlayer, isHost, countdownActive })) return;
        onSetSpectator?.(draggedId, true);
      });
    }

    const header = document.createElement("header");
    header.className = "lobby-spectator-header";
    const eye = document.createElement("span");
    eye.className = "lobby-observer-icon";
    eye.setAttribute("aria-hidden", "true");
    const title = document.createElement("div");
    const kicker = document.createElement("span");
    kicker.className = "lobby-kicker";
    kicker.textContent = spectatorOnly ? "Replay lobby" : "Observers";
    const count = document.createElement("h2");
    count.textContent = spectatorOnly
      ? `${players.length} viewer${players.length === 1 ? "" : "s"}`
      : `${players.length} spectator${players.length === 1 ? "" : "s"}`;
    title.append(kicker, count);
    header.append(eye, title);

    const list = document.createElement("div");
    list.className = "lobby-observer-list";
    for (const player of players) {
      list.appendChild(this._buildSpectatorRow({
        player,
        myId,
        hostId,
        isHost: !spectatorOnly && isHost,
        countdownActive,
        spectatorOnly,
      }));
    }
    if (players.length === 0) {
      const empty = document.createElement("div");
      empty.className = "lobby-empty-spectators";
      empty.textContent = spectatorOnly ? "Waiting for viewers" : (isHost ? "Drop a player here" : "No observers");
      list.appendChild(empty);
    }

    section.append(header, list);
    return section;
  }

  _buildSpectatorRow({ player, myId, hostId, isHost, countdownActive, spectatorOnly = false }) {
    const row = document.createElement("div");
    row.className = "player-row lobby-observer-row is-spectator";
    if (player.id === myId) row.classList.add("is-you");
    if (isHost && !countdownActive) {
      row.draggable = true;
      row.addEventListener("dragstart", (ev) => {
        ev.dataTransfer?.setData("application/x-rts-player-id", String(player.id));
        ev.dataTransfer.effectAllowed = "move";
        row.classList.add("is-dragging");
      });
      row.addEventListener("dragend", () => row.classList.remove("is-dragging"));
    }

    const swatch = document.createElement("span");
    swatch.className = "player-color";
    swatch.style.background = player.color || "#888";
    swatch.setAttribute("aria-hidden", "true");

    const body = document.createElement("div");
    body.className = "lobby-seat-body";
    const nameLine = document.createElement("div");
    nameLine.className = "lobby-seat-name";
    const name = document.createElement("span");
    name.className = "player-name";
    name.textContent = player.name || `Player ${player.id}`;
    nameLine.appendChild(name);
    const tags = document.createElement("span");
    tags.className = "player-tags";
    if (player.id === hostId) tags.appendChild(tag("host", "Host"));
    tags.appendChild(tag("spectator", "Spectator"));
    nameLine.appendChild(tags);
    const meta = document.createElement("div");
    meta.className = "lobby-seat-meta";
    meta.textContent = spectatorOnly ? "Replay viewer" : "No command seat";
    body.append(nameLine, meta);

    const state = document.createElement("span");
    state.className = "player-ready spectator";
    state.textContent = "Observing";

    row.append(swatch, body, state);
    return row;
  }
}

function tag(kind, text) {
  const el = document.createElement("span");
  el.className = `tag ${kind}`;
  el.textContent = text;
  return el;
}

export function playableAiProfileId(id) {
  const canonicalId = AI_PROFILE_ALIASES[id] || id;
  return AI_PROFILES.some((entry) => entry.id === canonicalId)
    ? canonicalId
    : DEFAULT_AI_PROFILE_ID;
}

function aiProfileLabel(id) {
  const fallback =
    AI_PROFILES.find((entry) => entry.id === DEFAULT_AI_PROFILE_ID) || AI_PROFILES[0];
  return (
    AI_PROFILES.find((entry) => entry.id === playableAiProfileId(id))?.label ||
    fallback?.label ||
    "AI"
  );
}

export const PLAYABLE_FACTIONS = Object.freeze([
  { id: "kriegsia", label: "Kriegsia" },
  { id: "ekat", label: "Ekat" },
]);

function playableFactionId(factionId) {
  return PLAYABLE_FACTIONS.some((entry) => entry.id === factionId) ? factionId : "kriegsia";
}

function factionLabel(factionId) {
  const entry = PLAYABLE_FACTIONS.find((item) => item.id === factionId);
  return entry ? entry.label : "Kriegsia";
}
