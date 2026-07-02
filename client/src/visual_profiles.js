const VISUAL_PROFILE_ERROR_MESSAGES = Object.freeze({
  invalid: "Invalid visualProfile. Use letters, numbers, underscores, or dashes, up to 48 characters.",
  unknown: "Unknown visualProfile.",
});

const TRENCH_VARIANTS_1_STATIC_SAMPLES = Object.freeze([
  Object.freeze({
    id: "trench-a-basin",
    kind: "trench",
    label: "A Basin",
    x: 760,
    y: 640,
    radiusTiles: 0.375,
    variant: "basin",
  }),
  Object.freeze({
    id: "trench-b-wide-shadow",
    kind: "trench",
    label: "B Wide",
    x: 850,
    y: 640,
    radiusTiles: 0.375,
    variant: "wide_shadow",
  }),
  Object.freeze({
    id: "trench-c-hard-rim",
    kind: "trench",
    label: "C Rim",
    x: 940,
    y: 640,
    radiusTiles: 0.375,
    variant: "hard_rim",
  }),
  Object.freeze({
    id: "trench-d-broken-earth",
    kind: "trench",
    label: "D Broken",
    x: 1030,
    y: 640,
    radiusTiles: 0.375,
    variant: "broken_earth",
  }),
  Object.freeze({
    id: "trench-e-compact-dark",
    kind: "trench",
    label: "E Compact",
    x: 1120,
    y: 640,
    radiusTiles: 0.375,
    variant: "compact_dark",
  }),
]);

const VISUAL_PROFILE_ENTRIES = Object.freeze([
  Object.freeze({
    id: "trench-variants-1",
    label: "Trench variants 1",
    description: "Initial checked-in profile for local entrenchment visual candidates.",
    initialCamera: Object.freeze({ x: 960, y: 640, zoom: 0.9 }),
    staticSamples: TRENCH_VARIANTS_1_STATIC_SAMPLES,
  }),
]);

const VISUAL_PROFILE_BY_ID = new Map(VISUAL_PROFILE_ENTRIES.map((profile) => [profile.id, profile]));

function visualProfileError(code, profileId = "") {
  const message = code === "unknown" && profileId
    ? `${VISUAL_PROFILE_ERROR_MESSAGES.unknown} "${profileId}" is not registered.`
    : VISUAL_PROFILE_ERROR_MESSAGES[code] || "visualProfile could not be resolved.";
  return Object.freeze({ code, profileId, message });
}

export function visualProfileIds() {
  return VISUAL_PROFILE_ENTRIES.map((profile) => profile.id);
}

export function getVisualProfile(id) {
  return VISUAL_PROFILE_BY_ID.get(id) || null;
}

export function resolveVisualProfileLaunch(labLaunch, lookupProfile = getVisualProfile) {
  if (!labLaunch) return { profile: null, error: null };
  if (labLaunch.visualProfileError) {
    return {
      profile: null,
      error: visualProfileError(labLaunch.visualProfileError.code || "invalid"),
    };
  }
  const profileId = labLaunch.visualProfileId || "";
  if (!profileId) return { profile: null, error: null };
  const profile = lookupProfile(profileId);
  if (!profile) return { profile: null, error: visualProfileError("unknown", profileId) };
  return { profile, error: null };
}
