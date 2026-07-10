/** Describe how an armed Lab tool will consume the next pointer action. */
export function labToolDetailText(tool) {
  const clickRepeatedly = !!tool?.keepArmedOnWorldClick;
  const paintsOnDrag = !!tool?.paintOnDrag;
  const boxApplies = !!tool?.consumeBoxSelection;
  const boxRepeatedly = !!tool?.keepArmedOnBoxSelection;
  if (paintsOnDrag) {
    return "Click or drag to paint. Right-click or Esc cancels.";
  }
  if (boxApplies) {
    const cadence = clickRepeatedly || boxRepeatedly ? " repeatedly" : "";
    return `Click or drag-select to apply${cadence}. Right-click or Esc cancels.`;
  }
  return clickRepeatedly
    ? "Click the map to apply repeatedly. Drag-select, right-click, or Esc cancels."
    : "Click the map to apply. Drag-select, right-click, or Esc cancels.";
}
