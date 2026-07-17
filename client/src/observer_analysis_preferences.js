const STORAGE_KEY = "rts.observerAnalysisOverlay";
const LEGACY_STORAGE_KEY = "rts.replayAnalysisOverlay";

export const OBSERVER_ANALYSIS_TABS = Object.freeze([
  { id: "army-value", label: "Army value" },
  { id: "production", label: "Production" },
  { id: "research", label: "Research" },
  { id: "units", label: "Units" },
  { id: "resources", label: "Resources" },
  { id: "units-lost", label: "Units lost" },
  { id: "resources-lost", label: "Resources lost" },
]);

export function createObserverAnalysisOverlayPreferences(storage = safeLocalStorage()) {
  const fallback = {
    selectedTab: OBSERVER_ANALYSIS_TABS[0].id,
    visible: true,
    collapsed: false,
    position: null,
  };
  const state = { ...fallback, ...readStoredPreferences(storage) };
  normalizePreferences(state, fallback);

  return {
    get selectedTab() {
      return state.selectedTab;
    },
    set selectedTab(value) {
      state.selectedTab = isObserverAnalysisTabId(value) ? value : fallback.selectedTab;
      writeStoredPreferences(storage, state);
    },
    get visible() {
      return state.visible;
    },
    set visible(value) {
      state.visible = value !== false;
      writeStoredPreferences(storage, state);
    },
    get collapsed() {
      return state.collapsed;
    },
    set collapsed(value) {
      state.collapsed = value === true;
      writeStoredPreferences(storage, state);
    },
    get position() {
      return state.position ? { ...state.position } : null;
    },
    set position(value) {
      state.position = normalizePosition(value);
      writeStoredPreferences(storage, state);
    },
    clearPosition() {
      state.position = null;
      writeStoredPreferences(storage, state);
    },
    snapshot() {
      return { ...state, position: state.position ? { ...state.position } : null };
    },
  };
}

export function isObserverAnalysisTabId(id) {
  return OBSERVER_ANALYSIS_TABS.some((tab) => tab.id === id);
}

function normalizePreferences(state, fallback) {
  if (!isObserverAnalysisTabId(state.selectedTab)) state.selectedTab = fallback.selectedTab;
  state.visible = state.visible !== false;
  state.collapsed = state.collapsed === true;
  state.position = normalizePosition(state.position);
}

function normalizePosition(value) {
  const left = Number(value?.left);
  const top = Number(value?.top);
  return Number.isFinite(left) && Number.isFinite(top) ? { left, top } : null;
}

function safeLocalStorage() {
  try {
    return typeof window !== "undefined" ? window.localStorage : null;
  } catch {
    return null;
  }
}

function readStoredPreferences(storage) {
  if (!storage) return {};
  try {
    const raw = storage.getItem(STORAGE_KEY) || storage.getItem(LEGACY_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function writeStoredPreferences(storage, state) {
  if (!storage) return;
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage failures should not break observer viewing.
  }
}
