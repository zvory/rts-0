// Lobby view helpers: pure DOM rendering for pre-match teams, seats, and observers.
// The Lobby controller owns networking/state; this module owns structure.

export const TEAM_PRESETS = Object.freeze([
  { id: "ffa", label: "FFA", teams: [] },
  { id: "solo", label: "Solo", teams: [{ id: 1, cap: 1 }] },
  { id: "1v2", label: "1v2", teams: [{ id: 1, cap: 1 }, { id: 2, cap: 2 }] },
  { id: "1v3", label: "1v3", teams: [{ id: 1, cap: 1 }, { id: 2, cap: 3 }] },
  { id: "2v2", label: "2v2", teams: [{ id: 1, cap: 2 }, { id: 2, cap: 2 }] },
]);

export function presetById(id) {
  return TEAM_PRESETS.find((preset) => preset.id === id) || TEAM_PRESETS[0];
}

export function teamSlotsForPreset(presetId, players = []) {
  const preset = presetById(presetId);
  if (preset.id === "ffa") {
    return players
      .filter((player) => !player.isSpectator)
      .map((player) => ({ id: Number(player.teamId) || Number(player.id), cap: 1 }));
  }
  return preset.teams.map((team) => ({ ...team }));
}

export function splitLobbyPlayers(players = []) {
  return {
    seatedPlayers: players.filter((player) => !player.isSpectator),
    spectatorPlayers: players.filter((player) => player.isSpectator),
  };
}

export class LobbyRosterView {
  constructor(rootEl) {
    this.root = rootEl;
  }

  render({
    players,
    teamPreset,
    myId,
    hostId,
    isHost,
    countdownActive,
    playerCount,
    maxPlayers,
    onAddAi,
    onRemoveAi,
    onSetTeam,
  }) {
    if (!this.root) return;
    this.root.innerHTML = "";

    const { seatedPlayers, spectatorPlayers } = splitLobbyPlayers(players);
    const slots = teamSlotsForPreset(teamPreset, seatedPlayers);
    const renderedIds = new Set();

    for (const slot of slots) {
      const teamPlayers = seatedPlayers.filter((player) => Number(player.teamId) === Number(slot.id));
      this.root.appendChild(this._buildTeamColumn({
        slot,
        players: teamPlayers,
        slots,
        renderedIds,
        myId,
        hostId,
        isHost,
        countdownActive,
        playerCount,
        maxPlayers,
        onAddAi,
        onRemoveAi,
        onSetTeam,
      }));
    }

    for (const player of seatedPlayers.filter((candidate) => !renderedIds.has(candidate.id))) {
      const slot = { id: Number(player.teamId) || Number(player.id), cap: 1 };
      this.root.appendChild(this._buildTeamColumn({
        slot,
        players: [player],
        slots: [slot],
        renderedIds,
        myId,
        hostId,
        isHost,
        countdownActive,
        playerCount,
        maxPlayers,
        onAddAi,
        onRemoveAi,
        onSetTeam,
      }));
    }

    if (spectatorPlayers.length > 0) {
      this.root.appendChild(this._buildSpectatorSection(spectatorPlayers, myId, hostId));
    }
  }

  _buildTeamColumn({
    slot,
    players,
    slots,
    renderedIds,
    myId,
    hostId,
    isHost,
    countdownActive,
    playerCount,
    maxPlayers,
    onAddAi,
    onRemoveAi,
    onSetTeam,
  }) {
    const section = document.createElement("section");
    section.className = "lobby-team-card team-row";
    section.setAttribute("aria-label", `Team ${slot.id}`);

    const header = document.createElement("header");
    header.className = "lobby-team-header";

    const mark = document.createElement("span");
    mark.className = "lobby-team-mark";
    mark.setAttribute("aria-hidden", "true");

    const title = document.createElement("div");
    title.className = "lobby-team-title";
    const kicker = document.createElement("span");
    kicker.className = "lobby-kicker";
    kicker.textContent = teamKicker(slot.id);
    const name = document.createElement("h2");
    name.textContent = `Team ${slot.id}`;
    title.append(kicker, name);

    const count = document.createElement("span");
    count.className = "lobby-team-count team-row-count";
    count.textContent = `${players.length}/${slot.cap}`;

    header.append(mark, title, count);
    if (isHost) {
      const add = document.createElement("button");
      add.type = "button";
      add.className = "team-add-ai btn";
      add.textContent = "Add AI";
      add.title = `Add AI to Team ${slot.id}`;
      add.disabled = countdownActive || playerCount >= maxPlayers || players.length >= slot.cap;
      add.addEventListener("click", () => {
        if (!add.disabled) onAddAi?.(slot.id);
      });
      header.appendChild(add);
    }

    const seats = document.createElement("div");
    seats.className = "lobby-seat-list";
    for (const player of players) {
      renderedIds.add(player.id);
      seats.appendChild(this._buildSeatRow({
        player,
        slots,
        myId,
        hostId,
        isHost,
        countdownActive,
        onRemoveAi,
        onSetTeam,
      }));
    }

    section.append(header, seats);
    return section;
  }

  _buildSeatRow({ player, slots, myId, hostId, isHost, countdownActive, onRemoveAi, onSetTeam }) {
    const row = document.createElement("div");
    row.className = "player-row lobby-seat";
    if (player.id === myId) row.classList.add("is-you");
    if (player.isAi) row.classList.add("is-ai");

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

    const meta = document.createElement("div");
    meta.className = "lobby-seat-meta";
    meta.textContent = player.isAi ? "AI 1.0" : "Human commander";

    body.append(nameLine, meta);

    const controls = document.createElement("div");
    controls.className = "lobby-seat-controls";
    this._appendTeamAssignment(controls, player, slots, isHost, countdownActive, onSetTeam);
    controls.appendChild(this._buildReadyState(player, isHost, onRemoveAi));

    row.append(swatch, body, controls);
    return row;
  }

  _appendTeamAssignment(parent, player, slots, isHost, countdownActive, onSetTeam) {
    if (!isHost || slots.length <= 1) {
      const label = document.createElement("span");
      label.className = "player-team-label";
      label.textContent = `Team ${player.teamId}`;
      parent.appendChild(label);
      return;
    }

    const select = document.createElement("select");
    select.className = "player-team-select";
    select.setAttribute("aria-label", `Team for ${player.name || `Player ${player.id}`}`);
    select.disabled = countdownActive;
    for (const slot of slots) {
      const opt = document.createElement("option");
      opt.value = String(slot.id);
      opt.textContent = `Team ${slot.id}`;
      select.appendChild(opt);
    }
    select.value = String(player.teamId);
    select.addEventListener("change", () => {
      if (!select.disabled) onSetTeam?.(player.id, Number(select.value));
    });
    parent.appendChild(select);
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

  _buildSpectatorSection(players, myId, hostId) {
    const section = document.createElement("section");
    section.className = "lobby-spectator-card";
    section.setAttribute("aria-label", "Spectators");

    const header = document.createElement("header");
    header.className = "lobby-spectator-header";
    const eye = document.createElement("span");
    eye.className = "lobby-observer-icon";
    eye.setAttribute("aria-hidden", "true");
    const title = document.createElement("div");
    const kicker = document.createElement("span");
    kicker.className = "lobby-kicker";
    kicker.textContent = "Observers";
    const count = document.createElement("h2");
    count.textContent = `${players.length} spectator${players.length === 1 ? "" : "s"}`;
    title.append(kicker, count);
    header.append(eye, title);

    const list = document.createElement("div");
    list.className = "lobby-observer-list";
    for (const player of players) {
      list.appendChild(this._buildSpectatorRow(player, myId, hostId));
    }

    section.append(header, list);
    return section;
  }

  _buildSpectatorRow(player, myId, hostId) {
    const row = document.createElement("div");
    row.className = "player-row lobby-observer-row is-spectator";
    if (player.id === myId) row.classList.add("is-you");

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
    meta.textContent = "No command seat";
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

function teamKicker(teamId) {
  if (Number(teamId) === 1) return "Allied command";
  if (Number(teamId) === 2) return "Opposing command";
  return "Command group";
}
