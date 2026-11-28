import { KIND } from "./protocol.js";
import { LOADED_RIFLEMAN_RIG_KEY } from "./renderer/rigs/live_routing.js";
import { RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP } from "./renderer/rigs/rifleman_panzerfaust_png_strip.js";
import { SCOUT_PLANE_PNG_FRAME_STRIP } from "./renderer/rigs/scout_plane_png_strip.js";
import { RIFLEMAN_DIRECTIONAL_PROTOTYPE } from "./renderer/rigs/rifleman_directional_prototype.js";

const VISUAL_PROFILE_ERROR_MESSAGES = Object.freeze({
  invalid: "Invalid visualProfile. Use letters, numbers, underscores, or dashes, up to 48 characters.",
  unknown: "Unknown visualProfile.",
});

function cameraSnapshot(focusX, focusY, framingScale) {
  return Object.freeze({
    version: 1,
    focus: Object.freeze({ x: focusX, y: focusY }),
    framingScale,
    boundsPolicy: "mapOverscroll",
  });
}

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
  Object.freeze({
    id: "trench-f-empty-live",
    kind: "trench",
    label: "F Empty",
    x: 850,
    y: 730,
    radiusTiles: 0.375,
    variant: "basin",
  }),
  Object.freeze({
    id: "trench-g-occupied-lip",
    kind: "trench",
    label: "G Occupied",
    x: 940,
    y: 730,
    radiusTiles: 0.375,
    variant: "basin",
    occupied: true,
  }),
  Object.freeze({
    id: "trench-h-occupied-wide",
    kind: "trench",
    label: "H Wide Occ",
    x: 1030,
    y: 730,
    radiusTiles: 0.375,
    variant: "wide_shadow",
    occupied: true,
  }),
]);

const RIFLEMAN_RECOIL_FRAME_STRIP_OVERRIDES_1 = Object.freeze([
  Object.freeze({
    id: "rifleman-recoil-strip",
    label: "Rifleman recoil strip",
    kind: KIND.RIFLEMAN,
    strip: Object.freeze({
      enabled: true,
      unit: "rifleman",
      image: "/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-recoil-strip.png?v=pass02-recoil-two-frame-v3",
      imageVersion: "pass02-recoil-two-frame-v3",
      frameWidth: 96,
      frameHeight: 96,
      frameCount: 7,
      idleFrame: 0,
      movementFrames: [1, 2, 3, 4],
      firingFrames: [5, 6],
      firingFrameHoldPhase: 0.28,
      fps: 12,
      worldScale: 0.34,
      tintSlot: "team-light",
      bakedColorAdjustment: {
        brightness: 170,
        saturation: 118,
        hue: 100,
      },
    }),
  }),
]);

const RIFLEMAN_PANZERFAUST_COMPOSITED_FRAME_STRIP_OVERRIDES_1 = Object.freeze([
  Object.freeze({
    id: "rifleman-panzerfaust-composited-strip",
    label: "Rifleman Panzerfaust composited strip",
    kind: KIND.PANZERFAUST,
    rigKey: LOADED_RIFLEMAN_RIG_KEY,
    strip: RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP,
  }),
]);

const SCOUT_PLANE_FW189_FRAME_STRIP_OVERRIDES_1 = Object.freeze([
  Object.freeze({
    id: "scout-plane-fw189-pass-01",
    label: "Scout Plane Fw 189 pass 01",
    kind: KIND.SCOUT_PLANE,
    strip: SCOUT_PLANE_PNG_FRAME_STRIP,
  }),
]);

const GRASS_SURVEY_PROFILES = Object.freeze([
  [1, "Wind-swept pixel carpet", "Fine grass texture with coherent waves of wind-driven light."],
  [2, "Swaying blade tufts", "Sparse grass blade silhouettes bending together in the wind."],
  [3, "Painterly grass clumps", "Dense grouped grass clumps with coordinated swaying highlights."],
  [4, "Dry meadow", "Warm meadow grass and seed heads moving in a slower breeze."],
  [5, "Gusts and cloud shadows", "A textured grass carpet with broad gust fronts and passing shade."],
  [6, "Continuous short weave", "A dense pre-baked grass weave with wind distortion and passing shade."],
  [7, "Cross-woven carpet", "Two interleaved dense grass textures with layered wind motion and passing shade."],
  [8, "Cached blade animation", "A dense eight-frame blade atlas animated beneath gusts and passing shade."],
  [9, "Mottled turf beneath blades", "Profile 5 blades, gusts and shade over broad olive and sage ground patches."],
  [10, "Brushed turf beneath blades", "Profile 5 blades, gusts and shade over restrained flattened-turf strokes."],
  [11, "Earth and moss beneath blades", "Profile 5 blades, gusts and shade over muted earthy and mossy patches."],
  [12, "Infantry path wear prototype", "Lab-only cumulative infantry traffic: soft gray-brown dirt paths after repeated passes; no replay history."],
].map(([grassTechnique, label, description]) => Object.freeze({
  id: `grass-survey-${grassTechnique}`,
  label,
  description,
  grassTechnique,
  terrainPreviewReveal: true,
  initialCamera: cameraSnapshot(2050, 1930, 1.5),
})));

const VISUAL_PROFILE_ENTRIES = Object.freeze([
  Object.freeze({
    id: "rifleman-directional-prototype",
    label: "Rifleman 16-direction prototype",
    description: "Low-detail oblique Rifleman with registered foot anchors; slides without a walk cycle.",
    initialCamera: cameraSnapshot(2050, 1930, 2.1),
    frameStripOverrides: Object.freeze([Object.freeze({
      id: "rifleman-directional-prototype",
      kind: KIND.RIFLEMAN,
      strip: RIFLEMAN_DIRECTIONAL_PROTOTYPE,
    })]),
  }),
  ...GRASS_SURVEY_PROFILES,
  ...[
    [1, "Autotiled sandy bank", "Neighbor-selected edge and corner masks with sand and pebbles."],
    [2, "Muddy material splat", "Noisy alpha masks feather soil and stones into land and water."],
    [3, "Distance-field swash", "A sandy wet/dry bank with animated shore-following wavelets."],
  ].map(([shorelineTechnique, label, description]) => Object.freeze({
    id: `shoreline-study-${shorelineTechnique}`, label, description, shorelineTechnique,
    terrainPreviewReveal: true,
    initialCamera: cameraSnapshot(2750, 1900, 1.5),
  })),
  Object.freeze({
    id: "trench-variants-1",
    label: "Trench variants 1",
    description: "Checked-in profile for comparing empty and occupied entrenchment visuals.",
    initialCamera: cameraSnapshot(960, 690, 0.9),
    staticSamples: TRENCH_VARIANTS_1_STATIC_SAMPLES,
  }),
  Object.freeze({
    id: "rifleman-recoil-strip-1",
    label: "Rifleman recoil strip 1",
    description: "Local checked-in profile for previewing Rifleman firing recoil frames in the render-preview lab scenario.",
    initialCamera: cameraSnapshot(2050, 1930, 2.1),
    frameStripOverrides: RIFLEMAN_RECOIL_FRAME_STRIP_OVERRIDES_1,
  }),
  Object.freeze({
    id: "rifleman-panzerfaust-composite-1",
    label: "Rifleman Panzerfaust composite 1",
    description: "Local profile for previewing the white no-pack Rifleman with a deterministically animated Panzerfaust in the render-preview lab scenario.",
    initialCamera: cameraSnapshot(2050, 1930, 2.1),
    frameStripOverrides: RIFLEMAN_PANZERFAUST_COMPOSITED_FRAME_STRIP_OVERRIDES_1,
  }),
  Object.freeze({
    id: "scout-car-png-1",
    label: "Scout car PNG 1",
    description: "Local checked-in profile for previewing the Scout Car PNG body and rear machine gun in the render-preview lab scenario.",
    initialCamera: cameraSnapshot(2052, 1874, 2.4),
  }),
  Object.freeze({
    id: "mortar-png-1",
    label: "Mortar PNG 1",
    description: "Local checked-in profile for previewing the wheeled Mortar Team PNG carriage/tube atlas in the render-preview lab scenario.",
    initialCamera: cameraSnapshot(2052, 1952, 2.5),
  }),
  Object.freeze({
    id: "scout-plane-fw189-pass-01",
    label: "Scout Plane Fw 189 pass 01",
    description: "Local profile for previewing the generated detailed Fw 189 Scout Plane PNG with team tint in the render-preview lab scenario.",
    initialCamera: cameraSnapshot(2050, 1730, 2.8),
    frameStripOverrides: SCOUT_PLANE_FW189_FRAME_STRIP_OVERRIDES_1,
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
