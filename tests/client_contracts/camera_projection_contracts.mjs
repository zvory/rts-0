// Renderer-neutral semantic camera/projection contracts.

import {
  assert,
  assertApprox,
  assertDeepEqual,
  assertHasMethod,
  assertThrows,
} from "./assertions.mjs";
import { Camera } from "../../client/src/camera.js";
import {
  boundsForGroundPolygon,
  classifyProjectedPoint,
  clipGroundPolygonToBounds,
  PROJECTION_CLIP,
} from "../../client/src/camera_projection.js";

function rawView(camera) {
  return {
    x: camera.x,
    y: camera.y,
    zoom: camera.zoom,
    worldW: camera.worldW,
    worldH: camera.worldH,
    viewW: camera.viewW,
    viewH: camera.viewH,
  };
}

function polygonArea(points) {
  let area = 0;
  for (let index = 0; index < points.length; index += 1) {
    const next = (index + 1) % points.length;
    area += points[index].x * points[next].y - points[next].x * points[index].y;
  }
  return area / 2;
}

class FakePerspectiveProjection {
  constructor(frustumGround) {
    this.frustumGround = frustumGround;
    this.viewport = { widthCssPx: 200, heightCssPx: 120 };
    this.bounds = { minX: 0, minY: 0, maxX: 1000, maxY: 1000 };
  }

  project(point) {
    const depth = point.y;
    const safeDepth = depth === 0 ? 1 : Math.abs(depth);
    return classifyProjectedPoint({
      x: 100 + point.x * 100 / safeDepth,
      y: 90 - point.heightPx * 50 / safeDepth,
      depth,
    }, {
      ...this.viewport,
      nearDepth: 10,
      farDepth: 500,
    });
  }

  groundAtScreen(screen) {
    if (!Number.isFinite(screen?.x) || !Number.isFinite(screen?.y) || screen.y <= 20) return null;
    return Object.freeze({ x: (screen.x - 100) * 2, y: (screen.y - 20) * 5 });
  }

  viewportGroundPolygon() {
    return clipGroundPolygonToBounds(this.frustumGround, this.bounds);
  }

  viewportGroundBounds() {
    return boundsForGroundPolygon(this.viewportGroundPolygon());
  }
}

// Public semantic surface and orthographic equivalence.
// ---------------------------------------------------------------------------
{
  const camera = new Camera(800, 600, { minZoom: 0.25, maxZoom: 4 });
  camera.setBounds(2000, 1600, 800, 600);
  camera.setZoom(1.25);
  camera.centerOn(1000, 800);

  for (const method of [
    "project",
    "groundAtScreen",
    "projectedExtent",
    "viewportGroundPolygon",
    "viewportGroundBounds",
    "containsProjected",
    "focusAt",
    "framingForWorldPoints",
    "fitWorldPoints",
    "panByScreenDelta",
    "dollyBy",
    "resize",
    "setMapBounds",
    "snapshot",
    "restore",
    "audioListener",
    "subscribe",
    "projectionSnapshot",
  ]) assertHasMethod(camera, method, "Semantic Camera");

  const world = { x: 880, y: 760, heightPx: 0 };
  const legacyScreen = camera.worldToScreen(world.x, world.y);
  const projected = camera.project(world);
  assertApprox(projected.x, legacyScreen.x, 1e-9, "orthographic project x equals legacy transform");
  assertApprox(projected.y, legacyScreen.y, 1e-9, "orthographic project y equals legacy transform");
  assert(projected.depth > 0, "orthographic points have positive view depth");
  assert(projected.clip === PROJECTION_CLIP.INSIDE && projected.visible, "onscreen projection is visible");

  const elevated = camera.project({ ...world, heightPx: 50 });
  assertDeepEqual(elevated, projected, "orthographic projection intentionally ignores semantic height");
  const ground = camera.groundAtScreen(projected);
  assertApprox(ground.x, world.x, 1e-9, "orthographic ground projection round-trips x");
  assertApprox(ground.y, world.y, 1e-9, "orthographic ground projection round-trips y");
  assert(camera.groundAtScreen({ x: Number.NaN, y: 0 }) === null, "invalid ground query returns no hit");

  const outside = camera.project({ x: -1000, y: -1000, heightPx: 0 });
  assert(outside.clip === PROJECTION_CLIP.OUTSIDE_VIEWPORT, "offscreen orthographic point is clipped");
  assert(!camera.containsProjected({ x: -1000, y: -1000, heightPx: 0 }), "projected containment rejects far offscreen point");
  const extent = camera.projectedExtent(world, 20, 10);
  assertApprox(extent.width, 25, 1e-9, "projected width uses local CSS/world scale");
  assertApprox(extent.height, 12.5, 1e-9, "projected height uses local CSS/world scale");
  assertApprox(extent.scaleX, 1.25, 1e-9, "projected extent reports local x scale");
  assertApprox(extent.scaleY, 1.25, 1e-9, "projected extent reports local y scale");
  assert(extent.visible, "onscreen projected extent is visible");
}

// Point clip priority and fake-perspective depth/elevation/null-hit behavior.
// ---------------------------------------------------------------------------
{
  const viewport = { widthCssPx: 200, heightCssPx: 120, nearDepth: 10, farDepth: 500 };
  assert(
    classifyProjectedPoint({ x: -1, y: 50, depth: -2 }, viewport).clip
      === PROJECTION_CLIP.BEHIND_CAMERA,
    "behind-camera classification wins over viewport clipping",
  );
  assert(
    classifyProjectedPoint({ x: -1, y: 50, depth: 5 }, viewport).clip
      === PROJECTION_CLIP.OUTSIDE_DEPTH,
    "depth-limit classification wins over viewport clipping",
  );
  assert(
    classifyProjectedPoint({ x: -1, y: 50, depth: 100 }, viewport).clip
      === PROJECTION_CLIP.OUTSIDE_VIEWPORT,
    "positive in-range offscreen point uses viewport clip",
  );

  const fake = new FakePerspectiveProjection([
    { x: -100, y: 100 },
    { x: 700, y: -100 },
    { x: 1100, y: 700 },
    { x: 100, y: 1100 },
  ]);
  const ground = fake.project({ x: 0, y: 100, heightPx: 0 });
  const elevated = fake.project({ x: 0, y: 100, heightPx: 40 });
  assert(ground.visible, "fake perspective ground point is visible");
  assert(elevated.y < ground.y, "positive presentation height moves a perspective anchor upward");
  assert(
    fake.project({ x: 0, y: -10, heightPx: 0 }).clip === PROJECTION_CLIP.BEHIND_CAMERA,
    "fake perspective reports negative depth behind camera",
  );
  assert(
    fake.project({ x: 0, y: 900, heightPx: 0 }).clip === PROJECTION_CLIP.OUTSIDE_DEPTH,
    "fake perspective reports far clipping",
  );
  assert(fake.groundAtScreen({ x: 100, y: 10 }) === null, "fake perspective returns no hit above horizon");
  assert(fake.groundAtScreen({ x: 100, y: 60 })?.y === 200, "fake perspective returns finite ground below horizon");
}

// Bounded polygon semantics: clipping, deduplication, stable clockwise winding, and empties.
// ---------------------------------------------------------------------------
{
  const camera = new Camera(800, 600);
  camera.setBounds(2000, 1600, 800, 600);
  camera.centerOn(0, 0);
  const polygon = camera.viewportGroundPolygon();
  assert(polygon.length === 4, "partially overscrolled orthographic view clips to a rectangle");
  assert(polygon.every((point) => point.x >= 0 && point.y >= 0), "viewport polygon clips to map minimum bounds");
  assert(polygonArea(polygon) > 0, "world-y-down polygon winding is clockwise");
  assertDeepEqual(polygon[0], { x: 0, y: 0 }, "stable polygon starts at top-most then left-most point");
  assertDeepEqual(camera.viewportGroundBounds(), {
    minX: 0,
    minY: 0,
    maxX: 600,
    maxY: 450,
  }, "conservative ground bounds match clipped orthographic polygon");

  const deduped = clipGroundPolygonToBounds([
    { x: 0, y: 0 },
    { x: 0, y: 0 },
    { x: 10, y: 0 },
    { x: 10, y: 10 },
    { x: 0, y: 10 },
    { x: 0, y: 0 },
  ], { minX: 0, minY: 0, maxX: 10, maxY: 10 });
  assert(deduped.length === 4, "coincident polygon vertices are deduplicated");
  assert(Object.isFrozen(deduped) && deduped.every(Object.isFrozen), "ground polygon is immutable");

  const emptyFake = new FakePerspectiveProjection([
    { x: -50, y: -50 },
    { x: -10, y: -50 },
    { x: -10, y: -10 },
    { x: -50, y: -10 },
  ]);
  assertDeepEqual(emptyFake.viewportGroundPolygon(), [], "fully missed perspective ground view is empty");
  assert(emptyFake.viewportGroundBounds() === null, "empty ground polygon has no conservative bounds");

  const partialFake = new FakePerspectiveProjection([
    { x: -100, y: 100 },
    { x: 500, y: 100 },
    { x: 500, y: 500 },
    { x: -100, y: 500 },
  ]);
  assertDeepEqual(partialFake.viewportGroundBounds(), {
    minX: 0,
    minY: 100,
    maxX: 500,
    maxY: 500,
  }, "partial perspective ground view clips conservatively to map bounds");
}

// Semantic mutations: anchor-aware dolly, pan, focus, fit, clamping, resize, and listeners.
// ---------------------------------------------------------------------------
{
  const camera = new Camera(800, 600, { minZoom: 0.25, maxZoom: 4 });
  camera.setBounds(4000, 3000, 800, 600);
  camera.focusAt({ x: 2000, y: 1500 });
  const anchor = { x: 150, y: 220 };
  const beforeAnchor = camera.groundAtScreen(anchor);
  camera.dollyBy(2, anchor);
  const afterAnchor = camera.groundAtScreen(anchor);
  assertApprox(afterAnchor.x, beforeAnchor.x, 1e-9, "anchored dolly preserves ground x");
  assertApprox(afterAnchor.y, beforeAnchor.y, 1e-9, "anchored dolly preserves ground y");

  const beforePan = camera.snapshot().focus;
  camera.panByScreenDelta({ x: 40, y: -20 });
  const afterPan = camera.snapshot().focus;
  assertApprox(afterPan.x, beforePan.x - 20, 1e-9, "semantic screen pan preserves legacy x direction");
  assertApprox(afterPan.y, beforePan.y + 10, 1e-9, "semantic screen pan preserves legacy y direction");

  const beforeFit = camera.snapshot();
  const framing = camera.framingForWorldPoints([
    { x: 1000, y: 1000 },
    { x: 1400, y: 1200 },
    { x: Number.NaN, y: 0 },
  ], { paddingCssPx: 100 });
  assert(framing, "framing succeeds with finite world points");
  assertApprox(framing.framingScale, 1.5, 1e-9, "framing selects limiting CSS-pixel scale");
  assertDeepEqual(framing.focus, { x: 1200, y: 1100 }, "framing focuses finite point bounds");
  assertDeepEqual(camera.snapshot(), beforeFit, "framing calculation does not mutate the camera");
  assert(camera.fitWorldPoints([
    { x: 1000, y: 1000 },
    { x: 1400, y: 1200 },
    { x: Number.NaN, y: 0 },
  ], { paddingCssPx: 100 }), "fit succeeds with finite world points");
  assertApprox(camera.zoom, 1.5, 1e-9, "fit selects limiting CSS-pixel scale");
  assertDeepEqual(camera.snapshot().focus, { x: 1200, y: 1100 }, "fit focuses finite point bounds");
  const beforeEmptyFit = camera.snapshot();
  assert(!camera.fitWorldPoints([{ x: Number.NaN, y: 0 }]), "fit rejects a set with no finite points");
  assertDeepEqual(camera.snapshot(), beforeEmptyFit, "failed fit leaves semantic view unchanged");

  camera.focusAt({ x: -10000, y: -10000 });
  assert(camera.x >= -camera.viewW / camera.zoom / 4, "semantic focus preserves map overscroll clamp x");
  assert(camera.y >= -camera.viewH / camera.zoom / 4, "semantic focus preserves map overscroll clamp y");

  const notifications = [];
  const unsubscribe = camera.subscribe((snapshot) => notifications.push(snapshot));
  camera.resize(1000, 700);
  camera.setMapBounds(5000, 3500);
  camera.focusAt({ x: 2000, y: 1600 });
  assert(notifications.length === 3, "each successful semantic mutation emits one listener snapshot");
  assert(notifications.every((snapshot) => snapshot.version === 1), "listeners receive only CameraSnapshotV1");
  assert(notifications.every((snapshot) => Object.isFrozen(snapshot) && Object.isFrozen(snapshot.focus)), "listener snapshots are detached immutable values");
  unsubscribe();
  unsubscribe();
  camera.focusAt({ x: 2200, y: 1800 });
  assert(notifications.length === 3, "camera unsubscribe is idempotent and stops delivery");

  let duplicateCalls = 0;
  const duplicateListener = () => { duplicateCalls += 1; };
  const unsubscribeFirst = camera.subscribe(duplicateListener);
  const unsubscribeSecond = camera.subscribe(duplicateListener);
  camera.focusAt({ x: 2300, y: 1900 });
  assert(duplicateCalls === 2, "duplicate callbacks create independent subscriptions");
  unsubscribeFirst();
  camera.focusAt({ x: 2400, y: 2000 });
  assert(duplicateCalls === 3, "unsubscribing one duplicate preserves the other subscription");
  unsubscribeSecond();
}

// Versioned snapshot/legacy restore, listener data, and detached projection snapshots.
// ---------------------------------------------------------------------------
{
  const source = new Camera(800, 600, { minZoom: 0.25, maxZoom: 4 });
  source.setBounds(4000, 3000, 800, 600);
  source.setZoom(2);
  source.focusAt({ x: 1800, y: 1400 });
  const snapshot = source.snapshot();
  assertDeepEqual(snapshot, {
    version: 1,
    focus: { x: 1800, y: 1400 },
    framingScale: 2,
    boundsPolicy: "mapOverscroll",
  }, "semantic snapshot stores player intent rather than raw top-left state");
  assert(!Object.hasOwn(snapshot, "x") && !Object.hasOwn(snapshot, "zoom"), "semantic snapshot omits raw adapter state");

  const restored = new Camera(800, 600, { minZoom: 0.25, maxZoom: 4 });
  restored.setBounds(4000, 3000, 800, 600);
  assert(restored.restore(snapshot), "CameraSnapshotV1 restores successfully");
  assertDeepEqual(restored.snapshot(), snapshot, "semantic snapshot round-trips");

  assert(restored.restore({ x: 300, y: 200, zoom: 1.5 }), "named legacy top-left restore edge remains accepted");
  assertApprox(restored.x, 300, 1e-9, "legacy restore preserves raw orthographic x");
  assertApprox(restored.y, 200, 1e-9, "legacy restore preserves raw orthographic y");
  assert(restored.snapshot().version === 1, "legacy values are immediately normalized and never re-emitted");

  const beforeRejectedRestore = rawView(restored);
  assert(!restored.restore({ version: 2, focus: { x: 1, y: 2 }, framingScale: 1 }), "unknown snapshot version is rejected");
  assertDeepEqual(rawView(restored), beforeRejectedRestore, "rejected restore leaves raw view unchanged");

  const listener = restored.audioListener();
  assertApprox(listener.x, restored.snapshot().focus.x, 1e-9, "audio listener uses ground focus x");
  assertApprox(listener.y, restored.snapshot().focus.y, 1e-9, "audio listener uses ground focus y");
  assertApprox(listener.referenceDistancePx, restored.viewW / restored.zoom, 1e-9, "audio reference distance is one focus-plane viewport width in world px");

  const projection = restored.projectionSnapshot();
  const frozenPoint = projection.project({ x: 500, y: 400, heightPx: 0 });
  const frozenGround = projection.groundAtScreen({ x: frozenPoint.x, y: frozenPoint.y });
  restored.focusAt({ x: 3000, y: 2000 });
  assertDeepEqual(
    projection.groundAtScreen({ x: frozenPoint.x, y: frozenPoint.y }),
    frozenGround,
    "projection snapshot coefficients remain detached from later camera mutations",
  );
  assert(Object.isFrozen(projection) && Object.isFrozen(projection.viewport), "projection snapshot shape is immutable");
  assertDeepEqual(projection.snapshot(), projection.camera, "projection snapshot exposes its pinned CameraSnapshotV1");
}

// Finite-value rejection is non-mutating.
// ---------------------------------------------------------------------------
{
  const camera = new Camera(800, 600);
  camera.setBounds(2000, 1600, 800, 600);
  camera.focusAt({ x: 1000, y: 800 });
  const before = rawView(camera);
  assertThrows(() => camera.project({ x: Number.NaN, y: 1, heightPx: 0 }), "project rejects non-finite input");
  assertThrows(() => camera.project({ x: 1, y: 1, heightPx: null }), "project rejects non-number height");
  assertThrows(() => camera.dollyBy(0), "dolly rejects non-positive factor");
  assertThrows(() => camera.panByScreenDelta({ x: Number.POSITIVE_INFINITY, y: 0 }), "pan rejects non-finite delta");
  assertThrows(() => camera.resize(Number.NaN, 600), "resize rejects non-finite viewport");
  assertThrows(() => camera.setMapBounds(-1, 100), "map bounds reject negative extent");
  assertThrows(() => camera.containsProjected({ x: 1, y: 1, heightPx: 0 }, -1), "containment rejects negative CSS margin");
  assertDeepEqual(rawView(camera), before, "rejected semantic inputs never mutate camera state");

  camera.setZoom(camera.maxZoom);
  const overflowSnapshot = camera.projectionSnapshot();
  const point = { x: 1000, y: 800, heightPx: 0 };
  assertThrows(
    () => camera.projectedExtent(point, Number.MAX_VALUE, 1),
    "projected extent rejects derived overflow instead of returning infinity",
  );
  assertThrows(
    () => overflowSnapshot.projectedExtent(point, Number.MAX_VALUE, 1),
    "detached projected extent shares finite-output rejection",
  );
  camera.setZoom(camera.minZoom);
  assert(
    camera.groundAtScreen({ x: Number.MAX_VALUE, y: 0 }) === null,
    "ground hit returns null when finite screen input would overflow world coordinates",
  );
  assert(
    camera.projectionSnapshot().groundAtScreen({ x: Number.MAX_VALUE, y: 0 }) === null,
    "detached ground hit shares overflow-safe nullability",
  );
}

console.log("✅ camera_projection_contracts.mjs: semantic camera/projection contracts passed");
