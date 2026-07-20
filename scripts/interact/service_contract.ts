import type { InteractDriver } from "./driver.ts";

export type JsonObject = Record<string, unknown>;
export type EntityRef = string | number;

export interface AliasEntry { alias: string; id: number }
export interface ResolvedEntityRef { input: EntityRef; id: number; alias: string | null }
export interface SpawnInput extends JsonObject {
  owner: number;
  kind: string;
  x: number;
  y: number;
  alias?: string;
}
export interface ViewportInput { width: number; height: number; deviceScaleFactor?: number }
export interface ServiceInput extends JsonObject {
  sessionId?: string;
  workspaceRoot?: string;
  map?: string;
  seed?: string | number;
  scenario?: string;
  id?: string;
  unit?: string;
  count?: number;
  blocker?: string;
  case?: string;
  renderer?: string;
  viewport?: ViewportInput;
  categories?: string[];
  details?: boolean;
  spawns?: SpawnInput[];
  update?: JsonObject; updates?: JsonObject[]; refs?: EntityRef[]; ids?: number[];
  playerId?: number; command?: JsonObject; ignoreCommandLimits?: boolean;
  control?: JsonObject;
  kinds?: string[]; owners?: number[]; cameraViewport?: boolean; limit?: number;
  camera?: JsonObject;
  button?: "left" | "right";
  from?: { x: number; y: number };
  to?: { x: number; y: number };
  steps?: number;
  durationMs?: number;
  holdKeys?: Array<"attack" | "shift">;
  name?: string;
  presentation?: "clean" | "normal";
  subjects?: EntityRef[];
  kind?: string;
  artifactId?: string;
  path?: string;
  maxDurationMs?: number;
  crop?: { x: number; y: number; width: number; height: number };
  region?: "viewport" | "minimap" | { x: number; y: number; width: number; height: number };
  scale?: number;
  resumeSpeed?: number | null;
  fps?: number;
  frameCount?: number;
  reproduction?: boolean;
  opponent?: string;
  spectate?: string[]; autoSpectator?: boolean; sampleEveryMs?: number; speed?: number;
  ownership?: string;
  units?: number[];
  x?: number;
  y?: number;
  queued?: boolean;
}

export interface InteractSession {
  sessionId: string;
  kind: "lab" | "game" | "scenario";
  driver: InteractDriver;
  aliases: Map<string, number>;
  sceneRevision: number;
  sceneIdentity: JsonObject;
}

export class InteractError extends Error {
  details: JsonObject;
  code: string;
  constructor(code: string, message: string, details: JsonObject = {}) {
    super(message);
    this.name = "InteractError";
    this.code = code;
    this.details = details;
  }
}
