import type { InteractDriver } from "./driver.ts";
import { captureGameScreenshot } from "./game_screenshot.ts";
import { gameInspectionOwnership, requireGameSpectator } from "./game_session.ts";
import type { InteractTailnetPreview } from "./tailnet_preview.ts";

type JsonObject = Record<string, unknown>;
interface ObservationInput extends JsonObject {
  ids?: number[];
  kinds?: string[];
  ownership?: string;
  cameraViewport?: boolean;
  limit?: number;
  camera?: JsonObject;
  name?: string;
  presentation?: "clean" | "normal";
}
interface ObservationSession {
  sessionId: string;
  kind: "lab" | "game" | "scenario";
  driver: InteractDriver;
  sceneIdentity: JsonObject;
}

export async function executeObservationCommand(
  command: string,
  session: ObservationSession,
  input: ObservationInput,
  artifactPreview: InteractTailnetPreview | null,
) {
  const sessionId = session.sessionId;
  if (["game-inspect", "scenario-inspect"].includes(command)) {
    return handled("direct", { sessionId, ...await session.driver.inspect({
      ids: input.ids,
      kinds: input.kinds,
      ownership: gameInspectionOwnership(input.ownership, session.sceneIdentity.role),
      cameraViewport: input.cameraViewport === true,
      limit: input.limit || 25,
    }) });
  }
  if (["game-camera", "scenario-camera"].includes(command)) {
    const value = input.camera || {};
    const driverCommand = value.action === "focus"
      ? { action: "focus", entityIds: value.entities, padding: value.padding }
      : value;
    const response = await session.driver.camera(driverCommand);
    return handled("direct", {
      sessionId,
      camera: response.camera || response,
      cameraViewport: response.cameraViewport || null,
      cameraWorldBounds: response.cameraWorldBounds || null,
    });
  }
  if (command === "game-screenshot") {
    return handled("direct", await captureGameScreenshot(session, input, artifactPreview));
  }
  if (command === "scenario-screenshot") {
    return handled("direct", await captureGameScreenshot(
      session,
      { ...input, name: input.name || "scenario", presentation: input.presentation || "clean" },
      artifactPreview,
      "scenario",
    ));
  }
  if (["game-record-start", "scenario-record-start"].includes(command)) {
    const presentation = input.presentation || (session.kind === "scenario" ? "clean" : "normal");
    const recorder = await session.driver.recordStart({ ...input, sessionId, presentation });
    return handled("recording", { sessionId, recorder });
  }
  if (["game-capture-timelapse", "scenario-capture-timelapse"].includes(command)) {
    if (session.kind === "game") requireGameSpectator(session.sceneIdentity.role);
    const presentation = input.presentation || (session.kind === "scenario" ? "clean" : "normal");
    const capture = await session.driver.captureTimelapse({ ...input, sessionId, presentation });
    return handled("capture", capture);
  }
  return { handled: false as const };
}

function handled<const Kind extends string, Result>(kind: Kind, result: Result) {
  return { handled: true as const, kind, result };
}
