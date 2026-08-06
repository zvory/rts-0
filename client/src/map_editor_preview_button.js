/** Create the Map Editor document-bar control for hover preview and clipboard copy. */
export function createMapEditorPreviewButton({ session, onShow, onHide, onCopy, onStatus }) {
  const control = document.createElement("button");
  control.type = "button";
  control.className = "map-editor-button";
  control.textContent = "Preview";
  control.title = "Hover to preview the minimap; click to copy a 2048 px PNG.";
  control.setAttribute("aria-haspopup", "true");
  const payload = () => ({
    authoredMap: session.exportMap(),
    materializedMap: session.materialized(),
  });
  const show = () => {
    try { onShow?.(control, payload()); }
    catch (error) { onStatus(error?.message || String(error), true); }
  };
  const hide = () => onHide?.();
  control.addEventListener("pointerenter", show);
  control.addEventListener("pointerleave", hide);
  control.addEventListener("focus", show);
  control.addEventListener("blur", hide);
  control.addEventListener("click", async () => {
    try {
      await onCopy?.(payload());
      hide();
      onStatus("Copied the 2048 px minimap PNG to the clipboard.");
    } catch (error) {
      hide();
      onStatus(error?.message || String(error), true);
    }
  });
  return control;
}
