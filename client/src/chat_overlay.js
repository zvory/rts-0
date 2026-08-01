import { S } from "./protocol.js";

export const CHAT_FADE_MS = 8000;
export const MAX_VISIBLE_CHAT_MESSAGES = 6;

function normalizedTeamId(player) {
  const teamId = Number(player?.teamId);
  return Number.isInteger(teamId) && teamId > 0 ? teamId : Number(player?.id);
}

export function defaultGameChatChannel(players, playerId, spectator = false) {
  if (spectator) return "all";
  const local = (players || []).find((player) => Number(player?.id) === Number(playerId));
  if (!local) return "all";
  const localTeam = normalizedTeamId(local);
  return (players || []).some((player) =>
    player?.isAi !== true
      && Number(player?.id) !== Number(playerId)
      && normalizedTeamId(player) === localTeam,
  ) ? "team" : "all";
}

function isTextEntry(target) {
  const tag = String(target?.tagName || "").toUpperCase();
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target?.isContentEditable;
}

function consume(event) {
  event.preventDefault?.();
  event.stopImmediatePropagation?.();
  event.stopPropagation?.();
}

export class ChatOverlay {
  constructor({
    net,
    root,
    messages,
    composer,
    channelLabel,
    input,
    windowLike = globalThis.window,
    documentLike = globalThis.document,
    fadeMs = CHAT_FADE_MS,
  }) {
    this.net = net;
    this.root = root;
    this.messages = messages;
    this.composer = composer;
    this.channelLabel = channelLabel;
    this.input = input;
    this.windowLike = windowLike;
    this.documentLike = documentLike;
    this.fadeMs = fadeMs;
    this.enabled = false;
    this.scope = "lobby";
    this.room = "";
    this.readOnly = false;
    this.channels = ["all"];
    this.channel = "all";
    this.onOpenChange = null;
    this.timers = new Set();
    this.onChat = (message) => this.receive(message);
    this.onKeyDown = (event) => this.handleKeyDown(event);
    this.net.on(S.CHAT, this.onChat);
    this.windowLike?.addEventListener?.("keydown", this.onKeyDown, true);
  }

  setLobbyContext(payload) {
    const room = String(payload?.room || "");
    if (this.scope !== "lobby" || this.room !== room) {
      this.closeComposer();
      this.clearMessages();
    }
    this.enabled = !!room;
    this.scope = "lobby";
    this.room = room;
    this.readOnly = false;
    this.channels = ["all"];
    this.channel = "all";
    this.onOpenChange = null;
    this.sync();
  }

  setGameContext(payload, { onOpenChange = null } = {}) {
    this.closeComposer();
    this.clearMessages();
    this.enabled = true;
    this.scope = "game";
    this.room = "";
    this.readOnly = !!payload?.replay;
    const defaultChannel = defaultGameChatChannel(
      payload?.players,
      payload?.playerId,
      payload?.spectator,
    );
    this.channels = defaultChannel === "team" ? ["team", "all"] : ["all"];
    this.channel = defaultChannel;
    this.onOpenChange = onOpenChange;
    this.sync();
  }

  disable() {
    this.closeComposer();
    this.enabled = false;
    this.room = "";
    this.clearMessages();
    this.sync();
  }

  sync() {
    if (!this.root) return;
    this.root.hidden = !this.enabled;
    this.root.dataset.scope = this.scope;
    if (this.channelLabel) this.channelLabel.textContent = `[${this.channel.toUpperCase()}]`;
    if (this.input) this.input.placeholder = this.channel === "team" ? "Message allies…" : "Message all…";
  }

  handleKeyDown(event) {
    if (!this.enabled || event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
    const open = this.composer && !this.composer.hidden;
    if (!open) {
      if (event.code !== "Enter" || this.readOnly || isTextEntry(event.target)) return;
      consume(event);
      this.openComposer();
      return;
    }
    if (event.code === "Escape") {
      consume(event);
      this.closeComposer();
      return;
    }
    if (event.code === "Tab") {
      consume(event);
      if (this.channels.length > 1) {
        const index = this.channels.indexOf(this.channel);
        this.channel = this.channels[(index + 1) % this.channels.length];
        this.sync();
      }
      return;
    }
    if (event.code !== "Enter" || event.isComposing) return;
    consume(event);
    const text = String(this.input?.value || "").trim();
    if (text) this.net.chatSend(this.channel, text);
    this.closeComposer();
  }

  openComposer() {
    if (!this.enabled || this.readOnly || !this.composer) return;
    this.composer.hidden = false;
    this.documentLike?.exitPointerLock?.();
    this.onOpenChange?.(true);
    this.input?.focus?.();
  }

  closeComposer() {
    if (!this.composer || this.composer.hidden) return;
    this.composer.hidden = true;
    if (this.input) this.input.value = "";
    this.onOpenChange?.(false);
  }

  receive(message) {
    if (!this.enabled || message?.scope !== this.scope || !this.messages) return;
    const line = this.documentLike.createElement("div");
    line.className = `chat-message chat-message-${message.channel === "team" ? "team" : "all"}`;
    const prefix = message.channel === "team" ? "TEAM" : "ALL";
    line.textContent = `[${prefix}] ${String(message.senderName || "Commander")}: ${String(message.text || "")}`;
    this.messages.appendChild(line);
    while (this.messages.children.length > MAX_VISIBLE_CHAT_MESSAGES) {
      this.messages.children[0].remove();
    }
    const timer = this.windowLike.setTimeout(() => {
      this.timers.delete(timer);
      line.classList?.add?.("is-fading");
      const removeTimer = this.windowLike.setTimeout(() => {
        this.timers.delete(removeTimer);
        line.remove();
      }, 450);
      this.timers.add(removeTimer);
    }, this.fadeMs);
    this.timers.add(timer);
  }

  clearMessages() {
    for (const timer of this.timers) this.windowLike?.clearTimeout?.(timer);
    this.timers.clear();
    this.messages?.replaceChildren?.();
  }

  destroy() {
    this.closeComposer();
    this.clearMessages();
    this.net.off(S.CHAT, this.onChat);
    this.windowLike?.removeEventListener?.("keydown", this.onKeyDown, true);
  }
}
