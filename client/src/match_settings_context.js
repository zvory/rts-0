import { buildBackToLobbyAction, buildGiveUpAction, buildPauseAction, buildSettingsTabs } from "./settings_panels.js";
import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

export function buildMatchSettingsContextForMatch(match) {
  return buildMatchSettingsContext({
    replayViewer: match.replayViewer,
    labMetadata: match.labMetadata,
    state: match.state,
    capabilities: match.capabilities,
    livePauseState: match.livePauseState,
    giveUpSent: match.giveUpSent,
    audio: match.audio,
    hotkeyProfiles: match.hotkeyProfiles,
    prediction: match.prediction,
    predictionAdapter: match.predictionAdapter,
    input: match.input,
    onPauseGame: match.onPauseGame,
    onGiveUpOpen: match.onGiveUpOpen,
    onBackToLobby: match.onBackToLobby,
    onPredictionEnabledChange: match.onPredictionEnabledChange,
    onPointerLockToggle: match.onPointerLockToggle,
    onDebugPathToggle: match.onDebugPathToggle,
    onUnitRangeToggle: match.onUnitRangeToggle,
    livePauseActionLabel: () => match.livePauseActionLabel(),
    livePauseActionTitle: () => match.livePauseActionTitle(),
  });
}

export function buildMatchSettingsContext({
  replayViewer,
  labMetadata,
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
  onBackToLobby,
  onPredictionEnabledChange,
  onPointerLockToggle,
  onDebugPathToggle,
  onUnitRangeToggle,
  livePauseActionLabel,
  livePauseActionTitle,
}) {
  const lab = !!labMetadata;
  const spectator = !!state?.spectator || !!replayViewer;
  const kind = lab ? "lab" : replayViewer ? "replay" : spectator ? "spectator" : "match";
  const movementPathsAvailable = capabilities.diagnostics.movementPaths !== MOVEMENT_PATH_DIAGNOSTICS.NONE;
  const leaveAction = (replayViewer || lab)
    ? buildBackToLobbyAction({
      visible: typeof onBackToLobby === "function",
      onBackToLobby,
    })
    : buildGiveUpAction({
      visible: !spectator && !giveUpSent,
      onOpen: onGiveUpOpen,
    });
  return {
    kind,
    spectator,
    replay: !!replayViewer,
    actions: [
      buildPauseAction({
        visible: capabilities.matchControls?.pause && !livePauseState.paused,
        disabled: !livePauseState.canPause,
        label: livePauseActionLabel?.() || "Pause",
        title: livePauseActionTitle?.() || "",
        onPause: onPauseGame,
      }),
      leaveAction,
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
        unitRanges: state ? {
          state: () => ({
            hidden: false,
            available: true,
            enabled: !!state.showUnitRangesEnabled,
          }),
          onToggle: onUnitRangeToggle,
        } : null,
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
