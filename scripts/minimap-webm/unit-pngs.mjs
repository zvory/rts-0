// Optional offline presentation adapter; terrain, fog, buildings and notices stay in Minimap.
import fs from 'node:fs/promises';
import { liveUnitIconMarkupFor } from '../../client/src/renderer/rigs/unit_icon_sources.js';
import { inlineSvgImageSources } from '../../client/src/minimap_icon_image.js';
import { isUnit, isBuilding } from '../../client/src/protocol.js';

const SIZES = Object.freeze({
  worker: 9, rifleman: 9, panzerfaust: 10, machine_gunner: 12, tank: 26,
  scout_car: 18, command_car: 20, anti_tank_gun: 22, rocket_launcher: 25,
  scout_plane: 26, artillery: 26,
});

export function unitPngFacing(entity) {
  return (Number.isFinite(entity.facing) ? entity.facing : 0)
    - (entity.kind === 'machine_gunner' ? Math.PI / 2 : 0);
}

export function unitPngSize(kind, canvasSize = 480) {
  return (SIZES[kind] || 18) * 1.35 * canvasSize / 480;
}

export function installUnitPngs(view, { createCanvas, loadImage, rasterizeSvg }) {
  const minimap = view.minimap;
  const sprites = new Map();
  const originalBlip = minimap._drawEntityBlip;
  const originalOutline = minimap._drawPlayerOwnedEntityOutline;
  const teamColor = entity => view.state.players.find(p => p.id === entity.owner)?.color || '#0072b2';
  const keyFor = entity => `${entity.kind}:${teamColor(entity)}`;

  async function prepare(entities) {
    for (const entity of entities) {
      if (!isUnit(entity.kind)) continue;
      const key = keyFor(entity);
      if (sprites.has(key)) continue;
      const markup = await inlineSvgImageSources(
        liveUnitIconMarkupFor(entity.kind, { teamColor: teamColor(entity) }),
        async href => {
          const asset = new URL(`../../client/${href.replace(/^\//, '')}`, import.meta.url);
          return `data:image/png;base64,${(await fs.readFile(asset)).toString('base64')}`;
        },
      );
      // Native Canvas does not correctly implement the SVG tint filters used by HUD portraits.
      const image = await loadImage(await rasterizeSvg(Buffer.from(markup)));
      const mask = createCanvas(image.width, image.height);
      const context = mask.getContext('2d');
      context.drawImage(image, 0, 0);
      context.globalCompositeOperation = 'source-in';
      context.fillStyle = '#ffffff';
      context.fillRect(0, 0, image.width, image.height);
      sprites.set(key, { image, mask });
    }
  }

  minimap._drawPlayerOwnedEntityOutline = function (entities) {
    originalOutline.call(this, entities.filter(entity => isBuilding(entity.kind)));
  };
  minimap._drawEntityBlip = function (context, entity, ...args) {
    if (!isUnit(entity.kind)) return originalBlip.call(this, context, entity, ...args);
    const sprite = sprites.get(keyFor(entity));
    if (!sprite) throw Error(`Unit PNG was not prepared: ${entity.kind}`);
    const { image, mask } = sprite;
    const point = this._worldToCanvas(entity.x, entity.y);
    const scale = unitPngSize(entity.kind, view.canvas.width) / Math.max(image.width, image.height);
    const width = image.width * scale, height = image.height * scale;
    const outline = view.canvas.width / 480;
    context.save();
    context.translate(point.x, point.y);
    context.rotate(unitPngFacing(entity));
    // One-pixel silhouette dilation at 480px, rotating with the portrait.
    for (let i = 0; i < 16; i++) {
      const angle = i * Math.PI / 8;
      context.drawImage(mask, -width / 2 + Math.cos(angle) * outline,
        -height / 2 + Math.sin(angle) * outline, width, height);
    }
    context.drawImage(image, -width / 2, -height / 2, width, height);
    context.restore();
  };

  return {
    prepare,
    destroy() {
      minimap._drawEntityBlip = originalBlip;
      minimap._drawPlayerOwnedEntityOutline = originalOutline;
      sprites.clear();
    },
  };
}
