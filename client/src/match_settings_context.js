import { buildGiveUpAction, buildPauseAction, buildSettingsTabs } from "./settings_panels.js";
import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

export function buildMatchSettingsContext({
  replayViewer,
  state,
  capabilities,
  livePauseState,
  giveUpSent,
  audio,
  hotkeyProfiles,
  prediction,
  predictionAdapter,
  input,
  onPauseGame,
  onGiveUpOpen,
  onPredictionEnabledChange,
  onPointerLockToggle,
  onDebugPathToggle,
  livePauseActionLabel,
  livePauseActionTitle,
}) {
  const spectator = !!state?.spectator || !!replayViewer;
  const kind = replayViewer ? "replay" : spectator ? "spectator" : "match";
  const movementPathsAvailable = capabilities.diagnostics.movementPaths !== MOVEMENT_PATH_DIAGNOSTICS.NONE;
  return {
    kind,
    spectator,
    replay: !!replayViewer,
    actions: [
      buildPauseAction({
        visible: !spectator && capabilities.matchControls?.pause && !livePauseState.paused,
        disabled: !livePauseState.canPause,
        label: livePauseActionLabel?.() || "Pause",
        title: livePauseActionTitle?.() || "",
        onPause: onPauseGame,
      }),
      buildGiveUpAction({
        visible: !spectator && !giveUpSent,
        onOpen: onGiveUpOpen,
      }),
    ],
    tabs: buildSettingsTabs({
      audio,
      hotkeyProfiles,
      game: {
        kind,
        spectator,
        prediction: {
          state: () => ({
            hidden: spectator || !!replayViewer,
            enabled: !!prediction.enabled,
            active: !!prediction.enabled && !!predictionAdapter?.ready,
            pending: !!prediction.enabled && !!predictionAdapter?.loading,
            available: !replayViewer && !state?.spectator,
          }),
          onToggle: () => onPredictionEnabledChange?.(!prediction.enabled),
        },
        pointerLock: replayViewer ? null : {
          state: () => ({
            hidden: false,
            supported: !!input?.pointerLockSupported(),
            enabled: !!input?.pointerLocked,
            locked: !!input?.pointerLocked,
          }),
          onToggle: onPointerLockToggle,
        },
      },
      debug: {
        available: movementPathsAvailable,
        state: () => ({
          available: movementPathsAvailable,
          enabled: !!state?.debugPathOverlaysEnabled,
        }),
        onToggle: onDebugPathToggle,
      },
    }),
  };
}
