const PREDICTION_ENABLED_STORAGE_KEY = "rts.prediction.enabled";

export function readPredictionEnabled(storage = globalThis.localStorage) {
  try {
    return storage?.getItem(PREDICTION_ENABLED_STORAGE_KEY) !== "0";
  } catch {
    return true;
  }
}

export function writePredictionEnabled(enabled, storage = globalThis.localStorage) {
  try {
    if (enabled) storage?.removeItem(PREDICTION_ENABLED_STORAGE_KEY);
    else storage?.setItem(PREDICTION_ENABLED_STORAGE_KEY, "0");
  } catch {
    // Storage failures only make this preference session-local.
  }
}
