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
  if (this._drag.labToolConsumesBoxSelection) return;
  this._cancelActiveLabTool("boxSelect", { id: this._drag.labToolId });
  this._drag.labToolCancelled = true;
}

export function _consumeLabToolWorldClick(p, ev) {
  const intent = this._intent();
  const tool = intent?.activeLabTool || null;
  if (!tool) return false;
  const world = this._worldAt(p.x, p.y);
  const entity = _labToolEntityAtWorld.call(this, world, tool);
  const event = {
    tool,
    x: world.x,
    y: world.y,
    world,
    screen: { x: p.x, y: p.y },
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
