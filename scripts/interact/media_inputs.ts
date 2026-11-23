export const MEDIA_ID_RE = /^media_[a-f0-9]{32}$/;
export const MEDIA_PRESETS = Object.freeze({ compact: 38, balanced: 30, quality: 22 });
export type MediaInput = Record<string, unknown> & {
  artifactId?: string; artifacts?: string[]; labels?: string[];
  format?: "webm" | "mp4"; preset?: keyof typeof MEDIA_PRESETS;
  layout?: "grid" | "concat" | "before-after" | "difference";
  speed?: number; name?: string;
};
export function mediaError(code: string, message: string): Error & { code: string } {
  return Object.assign(new Error(message), { code });
}
export function validateMediaInput(command: string, value: unknown): MediaInput {
  const fail = (message: string): never => { throw mediaError("invalidInput", message); };
  if (!value || typeof value !== "object" || Array.isArray(value)) fail("Media input must be a JSON object.");
  const input = value as MediaInput;
  const fields = command === "media-compare"
    ? ["artifacts", "labels", "layout", "format", "preset", "speed", "name"]
    : command === "media-export" ? ["artifactId", "format", "preset", "speed", "name"] : ["artifactId"];
  if (!["media-export", "media-compare", "media-inspect", "media-publish"].includes(command)) fail("Unknown media command.");
  if (Object.keys(input).some(key => !fields.includes(key))) fail("Media input contains an unexpected field.");
  if (command === "media-compare") {
    if (!Array.isArray(input.artifacts) || input.artifacts.length < 2 || input.artifacts.length > 4 ||
      !input.artifacts.every(id => typeof id === "string" && MEDIA_ID_RE.test(id))) fail("Provide 2-4 media artifact IDs.");
    if (input.layout != null && !["grid", "concat", "before-after", "difference"].includes(input.layout)) fail("Unknown comparison layout.");
    if (["before-after", "difference"].includes(input.layout || "") && input.artifacts!.length !== 2) fail("This layout requires exactly two artifacts.");
    if (input.labels != null && (!Array.isArray(input.labels) || input.labels.length !== input.artifacts!.length ||
      !input.labels.every(label => typeof label === "string" && /^[A-Za-z0-9 ._()+:/-]{1,64}$/.test(label)))) fail("Provide one label per artifact, using 1-64 ASCII letters, numbers, spaces or ._()+:/-.");
  } else if (typeof input.artifactId !== "string" || !MEDIA_ID_RE.test(input.artifactId)) fail("Provide a media_<32-hex> artifactId.");
  if (input.format != null && !["webm", "mp4"].includes(input.format)) fail("format must be webm or mp4.");
  if (input.preset != null && !Object.hasOwn(MEDIA_PRESETS, input.preset)) fail("preset must be compact, balanced or quality.");
  if (input.speed != null && (typeof input.speed !== "number" || !Number.isFinite(input.speed) || input.speed < 0.25 || input.speed > 8)) fail("speed must be between 0.25 and 8.");
  if (input.name != null && (typeof input.name !== "string" || !/^[A-Za-z0-9_-]{1,48}$/.test(input.name))) fail("name must be a safe 1-48 character token.");
  return input;
}
