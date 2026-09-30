export function updateMinimapPointPreview(intent, mouseX, mouseY) {
  if (intent?.commandTarget !== "pointTanks") return false;
  intent.updatePointTargetPreview?.({ source: "minimap", mouseX, mouseY });
  intent.updateAntiTankGunSetupPreview?.(null);
  return true;
}
