export function arm(match, tool, callbacks = {}) {
  if (!match.clientIntent || typeof match.clientIntent.beginLabTool !== "function") return null;
  const onWorldClick = typeof callbacks === "function"
    ? callbacks
    : callbacks?.onWorldClick;
  const onBoxSelection = typeof callbacks === "object" ? callbacks?.onBoxSelection : null;
  match.labToolWorldClickHandler = typeof onWorldClick === "function" ? onWorldClick : null;
  match.labToolBoxSelectionHandler = typeof onBoxSelection === "function" ? onBoxSelection : null;
  const active = match.clientIntent.beginLabTool(tool);
  match.publishLabToolChange({ type: "armed", tool: active });
  return active;
}

export function updatePayload(match, payload) {
  const active = match.clientIntent?.updateLabToolPayload?.(payload) || null;
  if (active) match.publishLabToolChange({ type: "updated", tool: active });
  return active;
}

export function cancel(match, reason = "cancelled") {
  match.labToolWorldClickHandler = null;
  match.labToolBoxSelectionHandler = null;
  const cancelled = match.clientIntent?.cancelLabTool?.(reason) || null;
  if (cancelled) match.publishLabToolChange({ type: "cancelled", reason, tool: cancelled });
  return cancelled;
}

export function consumeWorldClick(match, event) {
  const tool = matchingActiveTool(match, event);
  if (!tool) return;
  try {
    handleAsyncResult(match, match.labToolWorldClickHandler?.({ ...event, tool }));
  } catch (err) {
    match.handleLabToolActionError(err);
  } finally {
    if (!tool.keepArmedOnWorldClick) match.cancelLabTool("worldClick");
  }
}

export function consumeBoxSelection(match, event) {
  const tool = matchingActiveTool(match, event);
  if (!tool) return;
  try {
    handleAsyncResult(match, match.labToolBoxSelectionHandler?.({ ...event, tool }));
  } catch (err) {
    match.handleLabToolActionError(err);
  } finally {
    if (!tool.keepArmedOnBoxSelection) match.cancelLabTool("boxSelect");
  }
}

function matchingActiveTool(match, event) {
  const tool = match.clientIntent?.activeLabTool || null;
  return tool && event?.tool?.id === tool.id ? tool : null;
}

function handleAsyncResult(match, result) {
  if (result && typeof result.catch === "function") {
    result.catch((err) => match.handleLabToolActionError(err));
  }
}
