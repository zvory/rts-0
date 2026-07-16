// tests/client_contracts/match_notice_presenter_contracts.mjs
// Match-scoped server notice presentation contracts.

import { assert } from "./assertions.mjs";
import { MatchNoticePresenter } from "../../client/src/match_notice_presenter.js";
import { NOTICE_SEVERITY } from "../../client/src/protocol.js";

function createSurfaces() {
  const toasts = [];
  const pings = [];
  let borderPulses = 0;
  const plays = [];
  return {
    toasts,
    pings,
    plays,
    toast: (text) => toasts.push(text),
    minimap: {
      ping: (x, y, severity, isUnderAttack) => pings.push({ x, y, severity, isUnderAttack }),
      pulseBorder: () => { borderPulses += 1; },
    },
    audio: {
      play(id, opts) {
        plays.push({ id, opts });
        return true;
      },
    },
    borderPulseCount: () => borderPulses,
  };
}

function underAttack(x, y) {
  return {
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x,
    y,
  };
}

{
  let now = 0;
  const surfaces = createSurfaces();
  const presenter = new MatchNoticePresenter({
    ...surfaces,
    isReplay: () => false,
    isSpectator: () => false,
    pointInViewport: () => false,
    now: () => now,
  });

  assert(presenter.present(underAttack(100, 100)), "first under-attack incident is admitted");
  now = 500;
  assert(!presenter.present(underAttack(120, 140)), "same-bucket incident repeat is suppressed");
  now = 1000;
  assert(presenter.present(underAttack(1100, 100)), "distinct under-attack bucket is admitted promptly");
  assert(surfaces.toasts.length === 2, "shared incident admission deduplicates toasts");
  assert(surfaces.pings.length === 2, "shared incident admission deduplicates minimap pings");
  assert(
    surfaces.pings.every(({ isUnderAttack }) => isUnderAttack === true),
    "under-attack minimap pings request the emphasized presentation",
  );
  assert(surfaces.plays.length === 2, "two admitted location buckets both schedule voices within 1.5 seconds");
  assert(
    surfaces.plays.every(({ id, opts }) =>
      id === "notice_under_attack" && opts.cooldownMs === 0 && opts.duck === true
    ),
    "admitted under-attack voices bypass generic cooldown and explicitly duck the mix",
  );

  presenter.present({ msg: "Not enough steel", severity: NOTICE_SEVERITY.INFO });
  const infoVoice = surfaces.plays.at(-1);
  assert(infoVoice.id === "notice_steel", "existing informational notice voice still plays");
  assert(infoVoice.opts.category === "ui", "informational notice keeps informational audio priority");
  assert(infoVoice.opts.duck === true, "informational spoken notice explicitly ducks the mix");
  presenter.present({ msg: "Not enough steel", severity: NOTICE_SEVERITY.INFO });
  assert(surfaces.toasts.length === 4, "ordinary informational notices are not incident-deduplicated");
}

{
  let now = 0;
  let inViewport = true;
  const surfaces = createSurfaces();
  const presenter = new MatchNoticePresenter({
    ...surfaces,
    isReplay: () => false,
    isSpectator: () => false,
    pointInViewport: () => inViewport,
    now: () => now,
  });

  assert(presenter.present(underAttack(100, 100)), "in-view incident is admitted for visual surfaces");
  inViewport = false;
  now = 1000;
  assert(!presenter.present(underAttack(120, 120)), "in-view incident consumes the shared cooldown");
  assert(surfaces.toasts.length === 1, "accepted in-view incident toasts once");
  assert(surfaces.pings.length === 1, "accepted in-view incident pings once");
  assert(surfaces.plays.length === 0, "accepted in-view under-attack incident stays silent");
}

{
  let replay = true;
  let spectator = false;
  const surfaces = createSurfaces();
  const presenter = new MatchNoticePresenter({
    ...surfaces,
    isReplay: () => replay,
    isSpectator: () => spectator,
    pointInViewport: () => false,
    now: () => 0,
  });

  presenter.present(underAttack(100, 100));
  replay = false;
  spectator = true;
  presenter.present(underAttack(1100, 100));
  assert(surfaces.toasts.length === 2, "replay and spectator notices still toast");
  assert(surfaces.pings.length === 2, "replay and spectator alerts still ping");
  assert(surfaces.plays.length === 0, "replay and spectator notices never play player audio");
}

console.log("✅ match_notice_presenter_contracts.mjs: all assertions passed");
