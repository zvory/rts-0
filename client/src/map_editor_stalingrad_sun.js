const DEG_TO_RAD = Math.PI / 180;
const RAD_TO_DEG = 180 / Math.PI;

// The U.S. Army Center of Military History dates the German spearhead's first arrival at the Volga
// north of Stalingrad to 23 August 1942. Solar declination follows the NOAA/Meeus approximation;
// local solar time intentionally avoids making a claim about wartime civil-clock conventions.
export const STALINGRAD_SUN_PRESET = Object.freeze({
  dateLabel: "23 Aug 1942",
  locationLabel: "steppe west of Stalingrad",
  latitudeDegrees: 48.7,
  longitudeDegrees: 44.3,
  dayOfYear: 235,
  minTimeHours: 5.25,
  maxTimeHours: 18.75,
  timeStepHours: 0.25,
});

export function boundedStalingradTime(value) {
  const numeric = Number(value);
  const fallback = 12;
  const bounded = Math.max(
    STALINGRAD_SUN_PRESET.minTimeHours,
    Math.min(STALINGRAD_SUN_PRESET.maxTimeHours, Number.isFinite(numeric) ? numeric : fallback),
  );
  return Math.round(bounded / STALINGRAD_SUN_PRESET.timeStepHours)
    * STALINGRAD_SUN_PRESET.timeStepHours;
}

export function stalingradSunAtTime(value) {
  const timeHours = boundedStalingradTime(value);
  const fractionalYear = 2 * Math.PI / 365
    * (STALINGRAD_SUN_PRESET.dayOfYear - 1 + (timeHours - 12) / 24);
  const declination = solarDeclination(fractionalYear);
  const latitude = STALINGRAD_SUN_PRESET.latitudeDegrees * DEG_TO_RAD;
  const hourAngle = (timeHours - 12) * 15 * DEG_TO_RAD;
  const elevationRadians = Math.asin(
    Math.sin(latitude) * Math.sin(declination)
    + Math.cos(latitude) * Math.cos(declination) * Math.cos(hourAngle),
  );
  const elevation = elevationRadians * RAD_TO_DEG;
  const azimuth = normalizeDegrees(
    Math.atan2(
      Math.sin(hourAngle),
      Math.cos(hourAngle) * Math.sin(latitude) - Math.tan(declination) * Math.cos(latitude),
    ) * RAD_TO_DEG + 180,
  );
  return Object.freeze({
    azimuthDegrees: Math.round(azimuth) % 360,
    elevationDegrees: Math.max(1, Math.min(89, Math.round(elevation))),
    warmth: Math.max(15, Math.min(100, Math.round(100 - elevation * 1.7))),
  });
}

export function stalingradTimeFromSun(sun) {
  if (!sun) return 12;
  let bestTime = 12;
  let bestScore = Infinity;
  for (let time = STALINGRAD_SUN_PRESET.minTimeHours;
    time <= STALINGRAD_SUN_PRESET.maxTimeHours;
    time += STALINGRAD_SUN_PRESET.timeStepHours) {
    const candidate = stalingradSunAtTime(time);
    const directionDelta = circularDegrees(candidate.azimuthDegrees, Number(sun.azimuthDegrees));
    const heightDelta = Math.abs(candidate.elevationDegrees - Number(sun.elevationDegrees));
    const score = directionDelta + heightDelta * 1.5;
    if (score < bestScore) {
      bestScore = score;
      bestTime = time;
    }
  }
  return bestTime;
}

export function formatStalingradTime(value) {
  const totalMinutes = Math.round(boundedStalingradTime(value) * 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

function solarDeclination(fractionalYear) {
  return 0.006918
    - 0.399912 * Math.cos(fractionalYear)
    + 0.070257 * Math.sin(fractionalYear)
    - 0.006758 * Math.cos(2 * fractionalYear)
    + 0.000907 * Math.sin(2 * fractionalYear)
    - 0.002697 * Math.cos(3 * fractionalYear)
    + 0.00148 * Math.sin(3 * fractionalYear);
}

function circularDegrees(left, right) {
  const delta = Math.abs(Number(left) - Number(right)) % 360;
  return Math.min(delta, 360 - delta);
}

function normalizeDegrees(value) {
  return (value % 360 + 360) % 360;
}
