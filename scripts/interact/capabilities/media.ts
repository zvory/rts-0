import type { InteractTailnetPreview } from "../tailnet_preview.ts";
import type { InteractSession, JsonObject, ServiceInput } from "../service_contract.ts";

const TAILNET_DELIVERY_INSTRUCTION = "Share this Tailnet URL with the user to preview the Interact artifact. Do not share a local file path.";

export async function startRecording(
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
  defaults: Partial<ServiceInput> = {},
) {
  const recorder = await session.driver.recordStart({ ...input, ...defaults, sessionId: session.sessionId });
  return { sessionId: session.sessionId, recorder: presentRecorderStatus(recorder, artifactPreview) };
}

export async function stopRecording(session: InteractSession, artifactPreview: InteractTailnetPreview | null) {
  const result = await session.driver.recordStop({
    aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
  });
  return presentRecordingResult({ sessionId: session.sessionId, ...result }, artifactPreview);
}

export async function waitForRecording(session: InteractSession, artifactPreview: InteractTailnetPreview | null) {
  const recording = await session.driver.recordWait();
  return presentRecordingResult({ sessionId: session.sessionId, ...recording }, artifactPreview);
}

export async function captureTimelapse(
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
  defaults: Partial<ServiceInput> = {},
) {
  const capture = await session.driver.captureTimelapse({ ...input, ...defaults, sessionId: session.sessionId });
  return presentFixedCaptureResult({ sessionId: session.sessionId, ...capture }, artifactPreview);
}

export async function captureFixed(
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
) {
  const result = await session.driver.captureFixed({
    ...input,
    sessionId: session.sessionId,
    sceneIdentity: session.sceneIdentity,
    sceneRevision: session.sceneRevision,
    aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
  });
  return presentFixedCaptureResult({ sessionId: session.sessionId, ...result }, artifactPreview);
}

export async function presentScreenshotResult(
  result: JsonObject & { pngPath: string; manifestPath: string },
  artifactPreview: InteractTailnetPreview | null,
) {
  if (!artifactPreview) return result;
  const { pngPath, manifestPath: _manifestPath, ...visible } = result;
  return {
    ...visible,
    preview: await publishPreview(artifactPreview, pngPath, "image/png"),
    manifest: { available: true, localPathWithheld: true },
  };
}

export async function presentRecordingResult(
  result: JsonObject & { videoPath: string; framePaths: string[]; contactSheetPath: string; manifestPath: string },
  artifactPreview: InteractTailnetPreview | null,
) {
  if (!artifactPreview) return result;
  const { videoPath, framePaths, contactSheetPath, manifestPath: _manifestPath, ...visible } = result;
  const [preview, poster] = await Promise.all([
    publishPreview(artifactPreview, videoPath, "video/mp4"),
    publishPreview(artifactPreview, contactSheetPath, "image/png"),
  ]);
  return {
    ...visible,
    preview: { ...preview, poster },
    frames: { count: Array.isArray(framePaths) ? framePaths.length : 0, localPathsWithheld: true },
    manifest: { available: true, localPathWithheld: true },
  };
}

export async function presentFixedCaptureResult(
  result: JsonObject & { videoPath: string; contactSheetPath: string; manifestPath: string; frameSummary: JsonObject },
  artifactPreview: InteractTailnetPreview | null,
) {
  if (!artifactPreview) return result;
  const { videoPath, contactSheetPath, manifestPath: _manifestPath, frameSummary, ...visible } = result;
  const [preview, poster] = await Promise.all([
    publishPreview(artifactPreview, videoPath, "video/mp4"),
    publishPreview(artifactPreview, contactSheetPath, "image/png"),
  ]);
  const { representativeFramePaths, ...summary } = frameSummary || {};
  return {
    ...visible,
    preview: { ...preview, poster },
    frameSummary: {
      ...summary,
      representativeFrames: Array.isArray(representativeFramePaths) ? representativeFramePaths.length : 0,
      localPathsWithheld: true,
    },
    manifest: { available: true, localPathWithheld: true },
  };
}

export function presentRecorderStatus(value: JsonObject, artifactPreview: InteractTailnetPreview | null) {
  if (!artifactPreview || !value || typeof value !== "object") return value;
  const { videoPath, last, ...status } = value;
  if (!last || typeof last !== "object") return videoPath == null ? status : { ...status, localPathWithheld: true };
  const { videoPath: lastVideoPath, framePaths, contactSheetPath, manifestPath, ...lastStatus } = last as JsonObject;
  return {
    ...status,
    ...(videoPath == null ? {} : { localPathWithheld: true }),
    last: {
      ...lastStatus,
      ...(lastVideoPath == null && !Array.isArray(framePaths) && contactSheetPath == null && manifestPath == null
        ? {}
        : { localPathsWithheld: true }),
    },
  };
}

async function publishPreview(artifactPreview: InteractTailnetPreview, filePath: string, mimeType: string) {
  try {
    const preview = await artifactPreview.publish({ filePath, mimeType });
    return { available: true, ...preview, instruction: TAILNET_DELIVERY_INSTRUCTION };
  } catch (error) {
    return {
      available: false,
      code: String(errorCode(error) || "tailnetPreviewUnavailable").slice(0, 80),
      message: conciseError(error),
      instruction: "Do not share a local file path. Restore Tailnet preview availability, then capture again.",
    };
  }
}

function errorCode(error: unknown): string | null {
  return isJsonObject(error) && typeof error.code === "string" ? error.code : null;
}

function conciseError(error: unknown) {
  return String(error instanceof Error ? error.message : "Interact command failed.").split("\nServer log tail:")[0].slice(0, 1000);
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
