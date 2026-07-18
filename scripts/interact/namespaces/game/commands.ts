import { controlCamera, inspectEntities, selectEntities } from "../../capabilities/observation.ts";
import { captureTimelapse, startRecording } from "../../capabilities/media.ts";
import { captureGameScreenshot } from "../../game_screenshot.ts";
import { gameInspectionOwnership, requireGameSpectator } from "../../game_session.ts";
import type { InteractTailnetPreview } from "../../tailnet_preview.ts";
import { InteractError } from "../../service_contract.ts";
import type { InteractSession, JsonObject, ServiceInput } from "../../service_contract.ts";

export async function executeGameCommand(
  command: string,
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
) {
  if (command === "game-inspect") {
    return inspectEntities(session, {
      ids: input.ids,
      kinds: input.kinds,
      ownership: gameInspectionOwnership(input.ownership, session.sceneIdentity.role),
      cameraViewport: input.cameraViewport === true,
      limit: input.limit || 25,
    });
  }
  if (command === "game-select") return selectEntities(session, input.ids || []);
  if (command === "game-camera") return controlCamera(session, cameraCommand(input.camera || {}));
  if (command === "game-screenshot") return captureGameScreenshot(session, input, artifactPreview);
  if (command === "game-record-start") {
    return startRecording(session, input, artifactPreview, { presentation: input.presentation || "normal" });
  }
  if (command === "game-capture-timelapse") {
    requireGameSpectator(session.sceneIdentity.role);
    return captureTimelapse(session, input, artifactPreview, { presentation: input.presentation || "normal" });
  }
  if (command === "game-move") {
    return { sessionId: session.sessionId, result: await session.driver.move({
      units: input.units || [], x: input.x, y: input.y, queued: input.queued === true,
    }) };
  }
  if (command === "game-give-up") return { sessionId: session.sessionId, result: await session.driver.giveUp() };
  throw new InteractError("unknownCommand", `Unknown game command ${command}.`);
}

function cameraCommand(value: JsonObject) {
  return value.action === "focus"
    ? { action: "focus", entityIds: value.entities, padding: value.padding }
    : value;
}
