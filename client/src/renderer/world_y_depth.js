/** Apply the shared back-to-front world-Y key used by unit bodies and tree canopies. */
export function applyWorldYDepth(display, record) {
  if (!display) return 0;
  const y = Number(record?.y);
  const depth = Number.isFinite(y) ? y : 0;
  display.zIndex = depth;
  return depth;
}
