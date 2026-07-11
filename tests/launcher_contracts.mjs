import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  GAME_ORIGINS,
  gameOrigin,
  gameProbeUrl,
  wakeChannel,
} from "../launcher/server.mjs";

assert.equal(gameOrigin("mainline"), GAME_ORIGINS.mainline);
assert.equal(gameOrigin("beta"), GAME_ORIGINS.beta);
assert.equal(GAME_ORIGINS.mainline, "https://bewegungskrieg-mainline.fly.dev");
assert.equal(GAME_ORIGINS.beta, "https://bewegungskrieg-beta.fly.dev");
assert.equal(gameOrigin("https://attacker.invalid"), null);
assert.equal(gameProbeUrl("https://attacker.invalid"), null);
assert.equal(gameProbeUrl("beta"), `${GAME_ORIGINS.beta}/version`);
assert.equal(gameProbeUrl("mainline"), `${GAME_ORIGINS.mainline}/version`);

const indexHtml = await readFile(new URL("../launcher/index.html", import.meta.url), "utf8");
assert.doesNotMatch(indexHtml, /https:\/\/(?:bewegungskrieg-(?:mainline|beta)\.fly\.dev|(?:mainline|beta)\.bewegungskrieg\.net)/,
  "the browser must use the server-authoritative redirect origin");

let probedUrl = null;
const probe = async (url) => {
  probedUrl = url;
  return { ok: true };
};
const rejected = await wakeChannel("https://attacker.invalid", {
  probe,
});
assert.equal(rejected.status, 400);
assert.equal(probedUrl, null, "invalid channels never reach the probe");

const launched = await wakeChannel("beta", { probe });
assert.equal(launched.status, 200);
assert.deepEqual(launched.body, { origin: GAME_ORIGINS.beta });
assert.equal(probedUrl, `${GAME_ORIGINS.beta}/version`);

console.log("launcher fixed-destination contract: ok");
