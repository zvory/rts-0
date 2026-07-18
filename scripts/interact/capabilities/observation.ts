import type { InteractSession, JsonObject } from "../service_contract.ts";

export async function inspectEntities(session: InteractSession, query: JsonObject = {}): Promise<JsonObject> {
  return { sessionId: session.sessionId, ...await session.driver.inspect(query) };
}

export async function selectEntities(session: InteractSession, entityIds: number[]): Promise<JsonObject> {
  return { sessionId: session.sessionId, ...await session.driver.select(entityIds) };
}

export async function controlCamera(session: InteractSession, command: JsonObject) {
  const response = await session.driver.camera(command);
  return {
    sessionId: session.sessionId,
    camera: response.camera || response,
    cameraViewport: response.cameraViewport || null,
    cameraWorldBounds: response.cameraWorldBounds || null,
  };
}
