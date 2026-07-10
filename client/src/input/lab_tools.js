import { clearPostQuickCastSelectionGuard } from "./quick_cast_selection_guard.js";
import { isUnit } from "../protocol.js";
import {
  _dragWorldRect,
  _selectableEntityIdsInDragRect,
  selectableEntity,
} from "./selection.js";

export function _beginLabToolClick(p, ev, activeLabTool) {
  clearPostQuickCastSelectionGuard(this);
  this._drag = {
    x0: p.x,
    y0: p.y,
    x1: p.x,
    y1: p.y,
    suppressPostQuickCastSelection: false,
    labToolId: activeLabTool.id,
    labToolPaintsOnDrag: labToolPaintsOnDrag(activeLabTool),
    labToolConsumesBoxSelection: labToolConsumesBoxSelection(activeLabTool),
  };
  this._dragging = false;
  void ev;
}

export function _cancelActiveLabTool(reason, expectedTool = null) {
  const intent = this._intent();
  const active = intent?.activeLabTool || null;
  if (!active) return null;
  if (expectedTool?.id && active.id !== expectedTool.id) return null;
  const cancelled = this.labToolController?.cancel?.(reason);
  return cancelled || intent.cancelLabTool?.(reason) || null;
}

export function _cancelLabToolForBoxSelect() {
  if (!this._drag?.labToolId || this._drag.labToolCancelled) return;
  if (this._drag.labToolPaintsOnDrag) return;
  if (this._drag.labToolConsumesBoxSelection) return;
  this._cancelActiveLabTool("boxSelect", { id: this._drag.labToolId });
  this._drag.labToolCancelled = true;
}

export function _consumeLabToolWorldClick(p, ev) {
  const intent = this._intent();
  const tool = intent?.activeLabTool || null;
  if (!tool) return false;
  const world = this._worldAt(p.x, p.y);
  return consumeLabToolWorldPoint.call(this, tool, world, p, ev);
}

/**
 * Paint every newly crossed map tile for a tool that explicitly opts into a
 * drag stroke. Sampling by tile avoids duplicate actions from high-rate pointer
 * events while interpolating through fast cursor movement.
 */
export function _paintLabToolStroke(drag, p, ev) {
  const tool = this._labTool();
  if (!tool || tool.id !== drag?.labToolId || !drag?.labToolPaintsOnDrag) return false;

  const fromScreen = drag.labToolLastPaintScreen || { x: drag.x0, y: drag.y0 };
  const from = this._worldAt(fromScreen.x, fromScreen.y);
  const to = this._worldAt(p.x, p.y);
  if (!finitePoint(from) || !finitePoint(to)) return false;

  const tileSize = labToolPaintTileSize(this);
  const steps = Math.max(1, Math.ceil(Math.max(Math.abs(to.x - from.x), Math.abs(to.y - from.y)) / tileSize));
  const painted = drag.labToolPaintCells || (drag.labToolPaintCells = new Set());
  for (let index = 0; index <= steps; index++) {
    const t = index / steps;
    const world = {
      x: from.x + (to.x - from.x) * t,
      y: from.y + (to.y - from.y) * t,
    };
    const tileX = Math.floor(world.x / tileSize);
    const tileY = Math.floor(world.y / tileSize);
    if (!labToolPaintTileInBounds(this, tileX, tileY)) continue;
    const key = `${tileX},${tileY}`;
    if (painted.has(key)) continue;
    painted.add(key);
    const center = {
      x: (tileX + 0.5) * tileSize,
      y: (tileY + 0.5) * tileSize,
    };
    if (!consumeLabToolWorldPoint.call(this, tool, center, p, ev)) break;
    if (this._labTool()?.id !== tool.id) break;
  }
  drag.labToolLastPaintScreen = { x: p.x, y: p.y };
  return true;
}

/** Refresh the renderer-facing lab tool ghost from the current world cursor. */
export function _refreshLabToolPreview() {
  const intent = this._intent();
  const tool = intent?.activeLabTool || null;
  if (!tool) {
    intent?.updateLabToolPreview?.(null);
    return null;
  }
  const screen = this.mouse;
  if (!finitePoint(screen)) {
    intent?.updateLabToolPreview?.(null);
    return null;
  }
  const world = this._worldAt(screen.x, screen.y);
  return intent?.updateLabToolPreview?.({ toolId: tool.id, x: world?.x, y: world?.y }) || null;
}

function consumeLabToolWorldPoint(tool, world, screen, ev) {
  const intent = this._intent();
  if (!tool || intent?.activeLabTool?.id !== tool.id || !finitePoint(world)) return false;
  const entity = _labToolEntityAtWorld.call(this, world, tool);
  const event = {
    tool,
    x: world.x,
    y: world.y,
    world,
    screen: { x: screen.x, y: screen.y },
    entity,
    entityId: entity?.id ?? null,
    entityIds: entity ? [entity.id] : [],
    originalEvent: ev,
  };
  try {
    this.labToolController?.consumeWorldClick?.(event);
  } finally {
    if (intent.activeLabTool?.id === tool.id && !tool.keepArmedOnWorldClick) {
      this._cancelActiveLabTool("worldClick", tool);
    }
  }
  return true;
}

export function _finishLabToolBoxSelection(drag, ev) {
  const tool = this._labTool();
  if (!tool || tool.id !== drag.labToolId || !labToolConsumesBoxSelection(tool)) return false;
  const worldRect = _dragWorldRect.call(this, drag);
  const event = {
    tool,
    entityIds: _labToolEntityIdsInDragRect.call(this, drag, tool),
    screenRect: dragScreenRect(drag),
    worldRect,
    originalEvent: ev,
  };
  try {
    this.labToolController?.consumeBoxSelection?.(event);
  } finally {
    if (this._labTool()?.id === tool.id && !tool.keepArmedOnBoxSelection) {
      this._cancelActiveLabTool("boxSelect", tool);
    }
  }
  return true;
}

export function _finishLabToolClick(drag, p, ev) {
  this._lastClick = null;
  if (this._labTool()?.id === drag.labToolId) {
    this._consumeLabToolWorldClick(p, ev);
  }
}

function _labToolEntityAtWorld(world, tool) {
  if (!this.state || typeof this.state.entitiesInterpolated !== "function") return null;
  if (typeof this._entityAtWorld !== "function") return null;
  const entity = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ true);
  return _labToolEntitySelectable.call(this, entity, tool) ? entity : null;
}

function _labToolEntityIdsInDragRect(drag, tool) {
  if (!this.state || typeof this.state.entitiesInterpolated !== "function") return [];
  return _selectableEntityIdsInDragRect.call(this, drag, {
    unitsOnly: labToolTargetsUnitsOnly(tool),
  });
}

function _labToolEntitySelectable(entity, tool) {
  if (!entity) return false;
  if (!this.state) return false;
  if (labToolTargetsUnitsOnly(tool) && !isUnit(entity.kind)) return false;
  return selectableEntity(this.state, entity, !!this.state?.spectator);
}

function labToolConsumesBoxSelection(tool) {
  return !!tool?.consumeBoxSelection;
}

function labToolPaintsOnDrag(tool) {
  return !!tool?.paintOnDrag;
}

function labToolPaintTileSize(input) {
  const tileSize = Number(input?.state?.map?.tileSize);
  return Number.isFinite(tileSize) && tileSize > 0 ? tileSize : 32;
}

function labToolPaintTileInBounds(input, tileX, tileY) {
  const map = input?.state?.map;
  const width = Number(map?.width);
  const height = Number(map?.height);
  if (!Number.isFinite(width) || !Number.isFinite(height)) return tileX >= 0 && tileY >= 0;
  return tileX >= 0 && tileY >= 0 && tileX < width && tileY < height;
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}

function labToolTargetsUnitsOnly(tool) {
  return !!tool?.payload?.unitsOnly;
}

function dragScreenRect(drag) {
  return {
    x: Math.min(drag.x0, drag.x1),
    y: Math.min(drag.y0, drag.y1),
    w: Math.abs(drag.x1 - drag.x0),
    h: Math.abs(drag.y1 - drag.y0),
  };
}
