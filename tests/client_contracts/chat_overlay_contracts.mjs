import { assert, assertDeepEqual } from "./assertions.mjs";
import {
  ChatOverlay,
  defaultGameChatChannel,
} from "../../client/src/chat_overlay.js";

class FakeElement {
  constructor(tagName = "div") {
    this.tagName = tagName.toUpperCase();
    this.hidden = false;
    this.value = "";
    this.textContent = "";
    this.placeholder = "";
    this.dataset = {};
    this.children = [];
    this.parentNode = null;
    this.listeners = {};
    this.classList = { add: (value) => { this.addedClass = value; } };
  }
  appendChild(child) {
    if (child.parentNode) {
      child.parentNode.children = child.parentNode.children.filter((candidate) => candidate !== child);
    }
    child.parentNode = this;
    child.remove = () => {
      this.children = this.children.filter((candidate) => candidate !== child);
      child.parentNode = null;
    };
    this.children.push(child);
    return child;
  }
  insertBefore(child, reference) {
    if (child.parentNode) {
      child.parentNode.children = child.parentNode.children.filter((candidate) => candidate !== child);
    }
    child.parentNode = this;
    const index = this.children.indexOf(reference);
    if (index < 0) this.children.push(child);
    else this.children.splice(index, 0, child);
  }
  replaceChildren() { this.children = []; }
  focus() { this.focused = true; }
  blur() { this.focused = false; }
  addEventListener(type, handler) { this.listeners[type] = handler; }
  removeEventListener(type, handler) {
    if (this.listeners[type] === handler) delete this.listeners[type];
  }
  click() { this.listeners.click?.({}); }
}

function key(code, target = null) {
  return {
    code,
    target,
    preventDefault() { this.prevented = true; },
    stopImmediatePropagation() { this.stopped = true; },
  };
}

assert(
  defaultGameChatChannel(
    [{ id: 1, teamId: 1 }, { id: 2, teamId: 2 }],
    1,
  ) === "all",
  "1v1 defaults to all chat",
);
assert(
  defaultGameChatChannel(
    [{ id: 1, teamId: 1 }, { id: 2, teamId: 1 }, { id: 3, teamId: 2 }],
    1,
  ) === "team",
  "team games default to ally chat",
);
assert(
  defaultGameChatChannel([{ id: 1, teamId: 1 }, { id: 2, teamId: 1 }], 99, true) === "all",
  "spectators default to all chat",
);
assert(
  defaultGameChatChannel([
    { id: 1, teamId: 1 },
    { id: 2, teamId: 1, isAi: true },
    { id: 3, teamId: 2 },
  ], 1) === "all",
  "an AI-only teammate does not select an undeliverable team-chat default",
);

{
  const handlers = new Map();
  const sent = [];
  const net = {
    on(type, handler) { handlers.set(type, handler); },
    off(type) { handlers.delete(type); },
    chatSend(channel, text) { sent.push({ channel, text }); },
  };
  const root = new FakeElement();
  const floatingHost = new FakeElement();
  floatingHost.appendChild(root);
  const lobbyDock = new FakeElement();
  const messages = new FakeElement();
  const composer = new FakeElement();
  composer.hidden = true;
  const channelLabel = new FakeElement();
  const input = new FakeElement("input");
  const sendButton = new FakeElement("button");
  const windowListeners = {};
  const windowLike = {
    addEventListener(type, handler) { windowListeners[type] = handler; },
    removeEventListener(type) { delete windowListeners[type]; },
    setTimeout() { return 1; },
    clearTimeout() {},
  };
  const created = [];
  const documentLike = {
    createElement(tag) {
      const element = new FakeElement(tag);
      created.push(element);
      return element;
    },
    exitPointerLock() { this.exitedPointerLock = true; },
  };
  const menuStates = [];
  const chat = new ChatOverlay({
    net,
    root,
    messages,
    composer,
    channelLabel,
    input,
    sendButton,
    lobbyDock,
    windowLike,
    documentLike,
  });
  chat.setGameContext({
    playerId: 1,
    players: [{ id: 1, teamId: 1 }, { id: 2, teamId: 1 }, { id: 3, teamId: 2 }],
  }, { onOpenChange: (open) => menuStates.push(open) });
  assert(chat.channel === "team", "composer starts on ally chat in a team game");

  windowListeners.keydown(key("Enter"));
  assert(!composer.hidden && input.focused, "Enter opens and focuses the direct-text composer");
  windowListeners.keydown(key("Tab", input));
  assert(chat.channel === "all" && channelLabel.textContent === "[ALL]", "Tab cycles to all chat");
  input.value = "push now";
  windowListeners.keydown(key("Enter", input));
  assertDeepEqual(sent, [{ channel: "all", text: "push now" }], "second Enter sends the selected channel");
  assert(composer.hidden, "sending closes the composer");
  assertDeepEqual(menuStates, [true, false], "chat typing participates in interactive input capture");

  handlers.get("chat")({
    scope: "game",
    channel: "team",
    senderName: "Player <2>",
    text: "literal <b>text</b>",
  });
  assert(
    messages.children[0]?.textContent === "[TEAM] Player <2>: literal <b>text</b>",
    "chat is rendered as literal text rather than HTML",
  );

  chat.setLobbyContext({ room: "branch-room" });
  assert(
    !root.hidden && chat.scope === "lobby" && !chat.readOnly && root.parentNode === lobbyDock,
    "lobby context docks writable chat in the room panel",
  );
  assert(!composer.hidden && input.placeholder === "Message lobby…",
    "docked lobby chat keeps its composer visible");
  input.value = "ready when you are";
  sendButton.click();
  assertDeepEqual(sent.at(-1), { channel: "all", text: "ready when you are" },
    "the docked lobby Send button uses ephemeral all-chat");

  chat.setGameContext({ replay: {}, playerId: 99, spectator: true, players: [] });
  assert(root.parentNode === floatingHost, "game context restores chat to the floating overlay host");
  windowListeners.keydown(key("Enter"));
  assert(composer.hidden, "replay chat is presentation-only");
  chat.destroy();
}
