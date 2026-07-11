import assert from "node:assert/strict";
import { GAME_ORIGINS, gameOrigin, gameProbeUrl } from "../launcher/server.mjs";

assert.equal(gameOrigin("mainline"), GAME_ORIGINS.mainline);
assert.equal(gameOrigin("beta"), GAME_ORIGINS.beta);
assert.equal(gameOrigin("https://attacker.invalid"), null);
assert.equal(gameProbeUrl("https://attacker.invalid"), null);
assert.equal(gameProbeUrl("beta"), `${GAME_ORIGINS.beta}/version`);
assert.equal(gameProbeUrl("mainline"), `${GAME_ORIGINS.mainline}/version`);

console.log("launcher fixed-destination contract: ok");
