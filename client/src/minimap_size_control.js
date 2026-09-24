const STORAGE_KEY = "rts.minimap.size";
let sizePreference;

function normalize(value) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.min(100, number)) : 0;
}

export function initializeMinimapSize() {
  if (sizePreference === undefined) {
    try {
      sizePreference = normalize(globalThis.localStorage?.getItem(STORAGE_KEY));
    } catch {
      sizePreference = 0;
    }
  }
  globalThis.document?.documentElement?.style.setProperty("--minimap-growth", String(sizePreference / 100));
}

export function renderMinimapSizeControl(root) {
  initializeMinimapSize();
  const row = document.createElement("label");
  row.className = "audio-slider";
  const label = document.createElement("span");
  label.className = "audio-slider-label";
  label.textContent = "Minimap size";
  const input = document.createElement("input");
  input.id = "minimap-size-slider";
  input.type = "range";
  input.min = "0";
  input.max = "100";
  input.step = "1";
  input.value = String(sizePreference);
  input.title = "From the default size to half the screen width or height, whichever fits.";
  const sync = () => {
    input.setAttribute("aria-valuetext", sizePreference === 0 ? "Default" : sizePreference === 100 ? "Maximum" : `${sizePreference}% toward maximum`);
  };
  input.addEventListener("input", () => {
    sizePreference = normalize(input.value);
    initializeMinimapSize();
    sync();
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, String(sizePreference));
    } catch {
      // Keep the preference for this session when storage is unavailable.
    }
  });
  sync();
  row.append(label, input);
  root.appendChild(row);
}
