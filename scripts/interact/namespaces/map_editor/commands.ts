import { presentScreenshotResult } from "../../capabilities/media.ts";
import type { InteractTailnetPreview } from "../../tailnet_preview.ts";
import { InteractError } from "../../service_contract.ts";
import type { InteractSession, JsonObject, ServiceInput } from "../../service_contract.ts";

export async function executeMapEditorCommand(
  command: string,
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
) {
  if (command === "map-editor-inspect") {
    return { sessionId: session.sessionId, ...await session.driver.inspect({}) };
  }
  if (command === "map-editor-camera") {
    const response = await session.driver.camera(input.camera || {});
    return {
      sessionId: session.sessionId,
      camera: response.camera || null,
      cameraViewport: response.cameraViewport || null,
      cameraWorldBounds: response.cameraWorldBounds || null,
      map: response.map || null,
    };
  }
  if (command === "map-editor-screenshot") {
    const name = input.name || "map-editor";
    const presentation = input.presentation || "normal";
    const capture = await session.driver.screenshot({
      sessionId: session.sessionId,
      name,
      presentation,
      viewport: input.viewport || null,
      region: "viewport",
      request: { command: "map-editor screenshot", sessionId: session.sessionId, name, presentation, viewport: input.viewport || null },
    });
    const image = capture.image && typeof capture.image === "object" ? capture.image as JsonObject : {};
    return presentScreenshotResult({
      sessionId: session.sessionId,
      presentation: capture.presentation,
      region: capture.region,
      image: {
        mimeType: image.mimeType,
        bytes: image.bytes,
        width: image.width,
        height: image.height,
      },
      readiness: capture.readiness,
      pngPath: String(capture.pngPath),
      manifestPath: String(capture.manifestPath),
    }, artifactPreview);
  }
  throw new InteractError("unknownCommand", `Unknown Map Editor command ${command}.`);
}
