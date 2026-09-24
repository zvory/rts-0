// Host adapter only. Every minimap pixel is drawn by the production Minimap class.
import fs from 'node:fs/promises';
import { Minimap } from '../../client/src/minimap.js';
import { GameState } from '../../client/src/state.js';
import { Fog } from '../../client/src/fog.js';
import { MatchNoticePresenter } from '../../client/src/match_notice_presenter.js';
import { EVENT, KIND } from '../../client/src/protocol.js';
import { liveUnitIconMarkupFor } from '../../client/src/renderer/rigs/unit_icon_sources.js';
import { inlineSvgImageSources } from '../../client/src/minimap_icon_image.js';

export function unpackTiles(bits, count) {
  const bytes = Buffer.from(bits, 'base64');
  if (bytes.length !== Math.ceil(count / 8)) throw Error('invalid fog grid size');
  return Uint8Array.from({ length: count }, (_, i) => (bytes[i >> 3] >> (i & 7)) & 1);
}

export async function createRegularMinimap(header, { createCanvas, loadImage, rasterizeSvg }, size = 480) {
  if (header.sampleSchema !== 2) throw Error('Recapture with sampleSchema 2: production rendering needs complete snapshots and real events.');
  const priorWindow = globalThis.window;
  const priorNow = Object.getOwnPropertyDescriptor(performance, 'now');
  let now = 0;
  // The browser normally supplies these host APIs; use game time during offline rendering.
  const host = new EventTarget();
  host.devicePixelRatio = size / 220;
  globalThis.window = host;
  Object.defineProperty(performance, 'now', { configurable: true, value: () => now });
  const restoreHost = () => {
    if (priorWindow === undefined) delete globalThis.window; else globalThis.window = priorWindow;
    if (priorNow) Object.defineProperty(performance, 'now', priorNow); else delete performance.now;
  };
  try {
    const canvas = createCanvas(220, 220);
    const events = new EventTarget();
    canvas.addEventListener = events.addEventListener.bind(events);
    canvas.removeEventListener = events.removeEventListener.bind(events);
    canvas.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, width: 220, height: 220 });
    canvas.ownerDocument = { createElement: () => createCanvas(1, 1) };
    const markup = await inlineSvgImageSources(liveUnitIconMarkupFor(KIND.ARTILLERY), async href => {
      const path = new URL(`../../client/${href.replace(/^\//, '')}`, import.meta.url);
      return `data:image/png;base64,${(await fs.readFile(path)).toString('base64')}`;
    });
    const artilleryIconImage = await loadImage(Buffer.from(markup));
    const state = new GameState(header.start, { renderClock: { now: () => now } });
    const map = state.map, cells = map.width * map.height;
    const fog = new Fog(map.width, map.height, map.terrain);
    const minimap = new Minimap(canvas, state, null, fog, null, null, {
      commandsEnabled: false, artilleryIconImage,
      loadUnitIcon: async (kind, teamColor) => {
        const svg = await inlineSvgImageSources(liveUnitIconMarkupFor(kind, { teamColor }), async href => {
          const asset = new URL(`../../client/${href.replace(/^\//, '')}`, import.meta.url);
          return `data:image/png;base64,${(await fs.readFile(asset)).toString('base64')}`;
        });
        return loadImage(await rasterizeSvg(Buffer.from(svg)));
      },
      staticCanvasFactory: () => createCanvas(1, 1),
    });
    // Same backing-canvas resize as high-resolution minimap capture; preserve 220px UI proportions.
    canvas.width = size; canvas.height = size;
    const notices = new MatchNoticePresenter({ minimap, isReplay: () => true, isSpectator: () => true, now: () => now });
    return {
      canvas, minimap, state, fog,
      async prepare(row) { await minimap.prepareUnitIcons(row.snapshot.entities); },
      render(row) {
        if (!row.snapshot || !row.visibleBits || !row.exploredBits) throw Error('incomplete production-render sample');
        const visibleTiles = unpackTiles(row.visibleBits, cells);
        const exploredTiles = unpackTiles(row.exploredBits, cells);
        now = row.tick * 1000 / header.tickRate;
        state.applySnapshot({ ...row.snapshot, visibleTiles, exploredTiles }, now);
        fog.update([], map.tileSize, visibleTiles, exploredTiles);
        for (const { tick, event } of row.timedEvents || []) {
          now = tick * 1000 / header.tickRate;
          if (event.e === EVENT.NOTICE) notices.present(event);
          else if (event.e === EVENT.ARTILLERY_FIRING) minimap.markArtilleryFiring(event);
        }
        now = row.tick * 1000 / header.tickRate;
        minimap.render();
      },
      destroy() { try { minimap.destroy(); } finally { restoreHost(); } },
    };
  } catch (error) { restoreHost(); throw error; }
}
