import { controlCamera, inspectEntities, selectEntities } from "../../capabilities/observation.ts";
import { captureTimelapse, startRecording } from "../../capabilities/media.ts";
import { captureGameScreenshot } from "../../game_screenshot.ts";
import type { InteractTailnetPreview } from "../../tailnet_preview.ts";
import { InteractError } from "../../service_contract.ts";
import type { InteractSession, JsonObject, ServiceInput } from "../../service_contract.ts";

export async function executeDevScenarioCommand(
  command: string,
  session: InteractSession,
  input: ServiceInput,
  artifactPreview: InteractTailnetPreview | null,
) {
  if (command === "scenario-inspect") {
    return inspectEntities(session, {
      ids: input.ids,
      kinds: input.kinds,
      ownership: "visible",
      cameraViewport: input.cameraViewport === true,
      limit: input.limit || 25,
    });
  }
  if (command === "scenario-select") return selectEntities(session, input.ids || []);
  if (command === "scenario-camera") return controlCamera(session, cameraCommand(input.camera || {}));
  if (command === "scenario-screenshot") {
    return captureGameScreenshot(
      session,
      { ...input, name: input.name || "scenario", presentation: input.presentation || "clean" },
      artifactPreview,
      "scenario",
    );
  }
  if (command === "scenario-record-start") {
    return startRecording(session, input, artifactPreview, { presentation: input.presentation || "clean" });
  }
  if (command === "scenario-capture-timelapse") {
    return captureTimelapse(session, input, artifactPreview, { presentation: input.presentation || "clean" });
  }
  throw new InteractError("unknownCommand", `Unknown dev-scenario command ${command}.`);
}

function cameraCommand(value: JsonObject) {
  return value.action === "focus"
    ? { action: "focus", entityIds: value.entities, padding: value.padding }
    : value;
}
