import assert from "node:assert/strict";

import { MapEditorSession } from "../../client/src/map_editor_session.js";

const tile = { x: 8, y: 8 };
const session = new MapEditorSession({ storage: null });
session.initializeBlank({ size: 16, playerCount: 2 });
session.beginOverlayStroke("Painted concealment");
session.paintOverlayTiles([tile], { concealment: true });
assert.equal(session.commitOverlayStroke(), true);
session.beginOverlayStroke("Painted forest");
session.paintForestTiles([tile], true);
assert.equal(session.commitOverlayStroke(), true);
session.beginOverlayStroke("Erased forest");
session.paintForestTiles([tile], false);
assert.equal(session.commitOverlayStroke(), true);
assert.deepEqual(session.exportMap().concealmentTiles, [tile],
  "erasing forest preserves an independently authored overlay that overlapped it");
