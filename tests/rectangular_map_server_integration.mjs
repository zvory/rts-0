// Focused live-server coverage for the rectangular demo map. This drives the ordinary lobby
// selection/countdown/start path so schema-catalog drift cannot leave a valid map usable only by
// direct loaders or editor code.
//
// Usage: start the server (`cd server && cargo run`), then
// `node tests/rectangular_map_server_integration.mjs`.
import {
  assertStartProtocol,
  closeClients,
  connectClient,
  createAssertions,
  readyPlayers,
  sleep,
  startMatch,
  uniqueRoom,
} from "./team_harness.mjs";

const MAP_NAME = "1v1 Wide";
const MAP_WIDTH = 252;
const MAP_HEIGHT = 126;
const MAP_TILE_COUNT = MAP_WIDTH * MAP_HEIGHT;

const assertions = createAssertions();
const { ok } = assertions;

function terrainAt(map, tileX, tileY) {
  if (tileX < 0 || tileY < 0 || tileX >= map.width || tileY >= map.height) return undefined;
  return map.terrain[tileY * map.width + tileX];
}

function assertRectangularStart(start, playerId) {
  assertStartProtocol(ok, start, { playerId, expectedPlayers: 2, spectator: false });
  const { map } = start;
  ok(map.width === MAP_WIDTH && map.height === MAP_HEIGHT,
    `RECT MAP: start preserves independent dimensions (${map.width}x${map.height})`);
  ok(map.width === map.height * 2,
    `RECT MAP: horizontal extent is exactly twice the vertical extent (${map.width}/${map.height})`);
  ok(map.terrain.length === MAP_TILE_COUNT,
    `RECT MAP: terrain has width*height cells (${map.terrain.length})`);

  // The wide fixture is grass around the centered original battlefield. Probe each independent
  // far bound, not only the final flat-array cell, so width/height swaps and square-map clamps are
  // caught by the live start payload.
  ok(terrainAt(map, MAP_WIDTH - 1, 0) === 0,
    `RECT MAP: far horizontal tile (${MAP_WIDTH - 1},0) is in bounds and grass`);
  ok(terrainAt(map, 0, MAP_HEIGHT - 1) === 0,
    `RECT MAP: far vertical tile (0,${MAP_HEIGHT - 1}) is in bounds and grass`);
  ok(terrainAt(map, MAP_WIDTH, 0) === undefined,
    `RECT MAP: x=${MAP_WIDTH} is outside the horizontal bound`);
  ok(terrainAt(map, 0, MAP_HEIGHT) === undefined,
    `RECT MAP: y=${MAP_HEIGHT} is outside the vertical bound`);

  ok(start.players.every((player) =>
    player.startTileX >= 0 && player.startTileX < map.width
      && player.startTileY >= 0 && player.startTileY < map.height),
  "RECT MAP: all authoritative player starts are inside independent map bounds");
  ok(map.resources.every((resource) =>
    resource.x >= 0 && resource.x < map.width * map.tileSize
      && resource.y >= 0 && resource.y < map.height * map.tileSize),
  "RECT MAP: all authoritative resource positions are inside independent world bounds");
}

function assertRectangularFog(snapshot) {
  const visible = snapshot.visibleTiles;
  const explored = snapshot.exploredTiles;
  ok(visible?.length === MAP_TILE_COUNT,
    `RECT MAP: visible fog has width*height cells (${visible?.length})`);
  ok(explored?.length === MAP_TILE_COUNT,
    `RECT MAP: explored fog has width*height cells (${explored?.length})`);

  const farHorizontal = MAP_WIDTH - 1;
  const farVertical = (MAP_HEIGHT - 1) * MAP_WIDTH;
  ok(visible?.[farHorizontal] === 0 && explored?.[farHorizontal] === 0,
    `RECT MAP: fog contains the unseen far horizontal tile at flat index ${farHorizontal}`);
  ok(visible?.[farVertical] === 0 && explored?.[farVertical] === 0,
    `RECT MAP: fog contains the unseen far vertical tile at flat index ${farVertical}`);
  ok(visible?.[MAP_TILE_COUNT] === undefined && explored?.[MAP_TILE_COUNT] === undefined,
    `RECT MAP: fog stops at the rectangular bottom-right bound (${MAP_TILE_COUNT} cells)`);
}

(async () => {
  const room = uniqueRoom("rect-map");
  const host = await connectClient("rect-host");
  host.send({ t: "join", name: "Wide Host", room });
  const initialLobby = await host.waitFor((message) => message.t === "lobby", 3000, "wide-map lobby");
  const catalogEntry = initialLobby.maps?.find((map) => map.name === MAP_NAME);
  ok(catalogEntry != null, `${MAP_NAME} is listed by the ordinary lobby schema catalog`);
  ok(catalogEntry?.minPlayers === 1 && catalogEntry?.maxPlayers === 2,
    `${MAP_NAME} advertises one to two active players (${catalogEntry?.minPlayers}-${catalogEntry?.maxPlayers})`);

  host.send({ t: "selectMap", map: MAP_NAME });
  const selectedLobby = await host.waitFor(
    (message) => message.t === "lobby" && message.map === MAP_NAME,
    3000,
    "wide-map selection",
  );
  ok(selectedLobby.map === MAP_NAME, `${MAP_NAME} is selectable by the host through the lobby`);

  const guest = await connectClient("rect-guest");
  guest.send({ t: "join", name: "Wide Guest", room });
  await host.waitFor(
    (message) => message.t === "lobby" && message.map === MAP_NAME && message.players.length === 2,
    3000,
    "wide-map two-player lobby",
  );
  await readyPlayers([host, guest]);
  const { starts } = await startMatch(host, [host, guest]);
  assertRectangularStart(starts[0], host.playerId);
  assertRectangularStart(starts[1], guest.playerId);

  const snapshots = await Promise.all([host, guest].map((client) => client.waitFor(
    (message) => message.t === "snapshot"
      && message.visibleTiles?.length > 0
      && message.exploredTiles?.length > 0,
    4000,
    `${client.tag} rectangular fog snapshot`,
  )));
  snapshots.forEach(assertRectangularFog);

  closeClients(host, guest);
  await sleep(100);
  if (assertions.failures > 0) console.log(`\n${assertions.failures} FAILURE(S)`);
  process.exit(assertions.failures === 0 ? 0 : 1);
})().catch((error) => {
  console.log("TEST ERROR:", error.message);
  process.exit(2);
});
