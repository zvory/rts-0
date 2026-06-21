import { clearPostQuickCastSelectionGuard } from "./quick_cast_selection_guard.js";

export function _beginLabToolClick(p, ev, activeLabTool) {
  clearPostQuickCastSelectionGuard(this);
  this._drag = {
    x0: p.x,
    y0: p.y,
    x1: p.x,
    y1: p.y,
    suppressPostQuickCastSelection: false,
    labToolId: activeLabTool.id,
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
  this._cancelActiveLabTool("boxSelect", { id: this._drag.labToolId });
  this._drag.labToolCancelled = true;
}

export function _consumeLabToolWorldClick(p, ev) {
  const intent = this._intent();
  const tool = intent?.activeLabTool || null;
  if (!tool) return false;
  const world = this._worldAt(p.x, p.y);
  const event = {
    tool,
    x: world.x,
    y: world.y,
    world,
    screen: { x: p.x, y: p.y },
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

export function _finishLabToolClick(drag, p, ev) {
  this._lastClick = null;
  if (this._labTool()?.id === drag.labToolId) {
    this._consumeLabToolWorldClick(p, ev);
  }
}
