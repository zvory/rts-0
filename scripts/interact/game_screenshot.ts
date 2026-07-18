import type { InteractTailnetPreview } from "./tailnet_preview.ts";
import type { InteractDriver } from "./driver.ts";
import type { CaptureRegion } from "./capture_region.ts";
import { presentScreenshotResult } from "./capabilities/media.ts";

type JsonObject = Record<string, unknown>;

interface GameSession {
  sessionId: string;
  driver: InteractDriver;
}

export async function captureGameScreenshot(
  session: GameSession,
  input: JsonObject,
  artifactPreview: InteractTailnetPreview | null,
  captureKind: "game" | "scenario" = "game",
) {
  const name = typeof input.name === "string" ? input.name : captureKind;
  const presentation = input.presentation === "clean" || (input.presentation == null && captureKind === "scenario")
    ? "clean"
    : "normal";
  const viewport = isObject(input.viewport)
    ? input.viewport as { width: number; height: number; deviceScaleFactor?: number }
    : null;
  const subjectIds = Array.isArray(input.subjects) ? input.subjects.map(Number) : [];
  const inspected = subjectIds.length
    ? await session.driver.inspect({ ids: subjectIds, ownership: "visible", limit: subjectIds.length })
    : { entities: [] };
  const entities = objectArray(inspected.entities);
  const entitiesById = new Map(entities.map((entity) => [Number(entity.id), entity]));
  const subjectSummaries = subjectIds.map((id) => entitiesById.get(id) || null);
  if (subjectSummaries.some((entity) => !entity)) throw codedError("unknownEntity", `${captureKind} screenshot subjects must exist in the current watcher snapshot.`);
  const capture = await session.driver.screenshot({
    sessionId: session.sessionId,
    name,
    presentation,
    viewport,
    region: (input.region || "viewport") as CaptureRegion,
    subjectIds,
    subjectSummaries,
    request: { command: `${captureKind} screenshot`, sessionId: session.sessionId, name, presentation, viewport, region: input.region || "viewport", subjects: subjectIds },
  });
  const image = asObject(capture.image);
  const visible = {
    sessionId: session.sessionId,
    presentation: capture.presentation,
    region: capture.region,
    image: { mimeType: image.mimeType, bytes: image.bytes, width: image.width, height: image.height },
    readiness: capture.readiness,
  };
  return presentScreenshotResult({
    ...visible,
    pngPath: String(capture.pngPath),
    manifestPath: String(capture.manifestPath),
  }, artifactPreview);
}

function objectArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter(isObject) : [];
}

function asObject(value: unknown): JsonObject {
  return isObject(value) ? value : {};
}

function isObject(value: unknown): value is JsonObject {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function codedError(code: string, message: string) {
  return Object.assign(new Error(message), { code });
}
