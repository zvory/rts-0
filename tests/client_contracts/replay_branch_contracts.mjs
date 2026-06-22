// tests/client_contracts/replay_branch_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";

// Replay branch staging
// ---------------------------------------------------------------------------
{
  const { BranchStaging } = await import("../../client/src/branch_staging.js");
  function fakeEl(tag = "div") {
    const el = {
      tagName: tag.toUpperCase(),
      children: [],
      dataset: {},
      style: {},
      hidden: false,
      disabled: false,
      textContent: "",
      className: "",
      classList: {
        add(cls) {
          if (!el.className.split(/\s+/).includes(cls)) el.className = `${el.className} ${cls}`.trim();
        },
        remove(cls) {
          el.className = el.className.split(/\s+/).filter((c) => c && c !== cls).join(" ");
        },
        contains(cls) {
          return el.className.split(/\s+/).includes(cls);
        },
      },
      setAttribute(name, value) {
        this[name] = value;
      },
      appendChild(child) {
        child.parentNode = this;
        this.children.push(child);
        return child;
      },
      append(...children) {
        for (const child of children) this.appendChild(child);
      },
      replaceChildren(...children) {
        this.children = [];
        for (const child of children) this.appendChild(child);
      },
      addEventListener(type, handler) {
        this[`on${type}`] = handler;
      },
      remove() {
        if (!this.parentNode) return;
        this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
        this.parentNode = null;
      },
    };
    return el;
  }
  const priorDocument = globalThis.document;
  const priorSetTimeout = globalThis.setTimeout;
  const priorClearTimeout = globalThis.clearTimeout;
  let nextTimer = 1;
  const timers = new Map();
  globalThis.document = { createElement: fakeEl };
  globalThis.setTimeout = (fn) => {
    const id = nextTimer++;
    timers.set(id, fn);
    return id;
  };
  globalThis.clearTimeout = (id) => timers.delete(id);
  const sent = [];
  const handlers = new Map();
  const net = {
    playerId: 10,
    on(type, handler) { handlers.set(type, handler); },
    off(type) { handlers.delete(type); },
    claimBranchSeat(id) { sent.push(["claim", id]); },
    releaseBranchSeat(id) { sent.push(["release", id]); },
    startBranch() { sent.push(["start"]); },
  };
  const root = fakeEl("section");
  const staging = new BranchStaging(root, net);
  staging.show();
  handlers.get("branchStaging")({
    t: "branchStaging",
    room: "__replay_branch__:abc",
    sourceTick: 1200,
    hostId: 10,
    canStart: false,
    seats: [
      { playerId: 1, name: "Alpha", color: "#4878c8" },
      { playerId: 2, name: "Bravo", color: "#c84848", claimantId: 11, claimantName: "Other" },
    ],
    occupants: [{ id: 10, name: "Me" }, { id: 11, name: "Other" }],
  });
  assert(root.classList.contains("branch-staging-active"), "branch staging marks active root");
  const box = root.children[0];
  assert(box.className === "branch-staging-box", "branch staging renders focused room box");
  const seatList = box.children.find((child) => child.className === "branch-seat-list");
  assert(seatList.children.length === 2, "branch staging renders original seats");
  const claimButton = seatList.children[0].children[2];
  claimButton.onclick();
  assert(sent[0][0] === "claim" && sent[0][1] === 1, "claim button sends branch seat claim");
  const startButton = box.children.find((child) => child.className === "branch-actions").children[0];
  assert(startButton.hidden === false, "host sees start button");
  assert(startButton.disabled === true, "start disabled until all seats claimed");
  handlers.get("matchCountdown")({
    t: "matchCountdown",
    durationMs: 3000,
    words: ["Drei!", "Zwei!", "Eins!"],
  });
  const countdown = root.children.find((child) => child.className.includes("match-countdown"));
  assert(countdown?.textContent === "Drei!", "branch staging renders the visible countdown overlay");
  staging.hide();
  assert(
    !root.children.some((child) => child.className.includes("match-countdown")),
    "branch staging clears countdown overlay when hidden",
  );
  staging.destroy();
  globalThis.document = priorDocument;
  globalThis.setTimeout = priorSetTimeout;
  globalThis.clearTimeout = priorClearTimeout;
}

// ---------------------------------------------------------------------------
