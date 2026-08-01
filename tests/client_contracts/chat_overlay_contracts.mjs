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
    this.classList = { add: (value) => { this.addedClass = value; } };
  }
  appendChild(child) {
    child.parent = this;
    child.remove = () => {
      this.children = this.children.filter((candidate) => candidate !== child);
    };
    this.children.push(child);
  }
  replaceChildren() { this.children = []; }
  focus() { this.focused = true; }
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

{
  const handlers = new Map();
  const sent = [];
  const net = {
    on(type, handler) { handlers.set(type, handler); },
    off(type) { handlers.delete(type); },
    chatSend(channel, text) { sent.push({ channel, text }); },
  };
  const root = new FakeElement();
  const messages = new FakeElement();
  const composer = new FakeElement();
  composer.hidden = true;
  const channelLabel = new FakeElement();
  const input = new FakeElement("input");
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

  chat.setGameContext({ replay: {}, playerId: 99, spectator: true, players: [] });
  windowListeners.keydown(key("Enter"));
  assert(composer.hidden, "replay chat is presentation-only");
  chat.destroy();
}
