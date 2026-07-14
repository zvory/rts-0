import { validatorFor } from "./command_inputs.ts";
import type { CommandInput } from "./command_inputs.ts";
import {
  RECORDING_REQUEST_TIMEOUT_MS, REQUEST_TIMEOUT_MS, STARTUP_REQUEST_TIMEOUT_MS,
} from "./runtime.ts";

export type CommandScope = "daemon" | "session";
export type CommandLane = "serialized" | "observation" | "cancellation" | "lifecycle";
export type TimeoutClass = "ordinary" | "startup" | "lifecycle-media";

type HelpExample = Readonly<Record<string, unknown>>;
interface DescriptorOptions {
  scope?: CommandScope;
  lane?: CommandLane;
  timeoutClass?: TimeoutClass;
  sceneMutation?: boolean;
  recordable?: boolean;
  variants?: string[];
  defaults?: string[];
  bounds?: string[];
  example?: HelpExample;
}

export interface CommandDefinition {
  name: string;
  scope: CommandScope;
  lane: CommandLane;
  timeoutClass: TimeoutClass;
  validator: (value: unknown) => CommandInput;
  handlerKey: string;
  sceneMutation: boolean;
  recordable: boolean;
  help: Readonly<{
    summary: string;
    acceptedShape: string;
    variants: string[];
    defaults: string[];
    bounds: string[];
    example?: HelpExample;
  }>;
}

function descriptor(summary: string, acceptedShape: string, {
  scope = "session", lane = "serialized", timeoutClass = "ordinary",
  sceneMutation = false, recordable = true,
  variants = [], defaults = [], bounds = [], example,
}: DescriptorOptions = {}) {
  return Object.freeze({
    scope, lane, timeoutClass, sceneMutation, recordable,
    help: Object.freeze({ summary, acceptedShape, variants, defaults, bounds, example }),
  });
}

const COMMAND_RECORDS = Object.freeze({
  open: descriptor(
    "Open or recover the one authoritative Interact session for this worktree.",
    "{workspaceRoot?:string,map?:token,seed?:string|u32,scenario?:token,renderer?:\"pixi\"|\"babylon\",viewport?:{width:int,height:int,deviceScaleFactor?:number}}",
    {
      scope: "daemon", lane: "lifecycle", timeoutClass: "startup", recordable: false,
      defaults: ["workspaceRoot=current worktree", "map=Default", "scenario=blank", "renderer=pixi", "seed=empty", "viewport=1440x900 at DPR 1"],
      bounds: ["one session", "map/scenario <=48 safe-token characters", "viewport 320-4096 x 240-4096", "DPR >0 and <=4"],
      example: { renderer: "babylon", viewport: { width: 1000, height: 700, deviceScaleFactor: 1 } },
    },
  ),
  close: descriptor("Close the active browser/server session without stopping the daemon.", "{sessionId:string}", {
    lane: "lifecycle", timeoutClass: "lifecycle-media", recordable: false,
    bounds: ["sessionId must be the current Interact session id"], example: { sessionId: "<lab-session-id>" },
  }),
  reset: descriptor("Reset the ephemeral Lab scene and reconcile aliases that still match.", "{sessionId:string}", {
    sceneMutation: true,
    bounds: ["sessionId must be current", "at most 400 aliases are reconciled"], example: { sessionId: "<lab-session-id>" },
  }),
  catalog: descriptor("List bounded maps, players, factions, kinds, upgrades, commands, or abilities.", "{sessionId:string,categories?:category[]}", {
    variants: ["category=maps|players|factions|units|buildings|upgrades|commands|abilities"],
    defaults: ["categories=all categories"], bounds: ["0-8 unique requested categories"],
    example: { sessionId: "<lab-session-id>", categories: ["players", "units", "commands"] },
  }),
  spawn: descriptor("Atomically spawn one bounded batch and optionally assign aliases.", "{sessionId:string,spawns:[{owner:u32,kind:token,x:number,y:number,completed?:boolean,alias?:alias}],details?:boolean}", {
    variants: ["details=false returns counts and at most 12 alias/id rows", "details=true returns every decorated entity and raw authoritative outcome"],
    sceneMutation: true,
    defaults: ["completed=true", "alias=none", "details=false"], bounds: ["1-400 spawns", "400 aliases per session", "alias [A-Za-z][A-Za-z0-9_-]{0,31}", "12 default response details"],
    example: { sessionId: "<lab-session-id>", spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "subject" }] },
  }),
  update: descriptor("Atomically apply a bounded batch of Lab scene updates.", "{sessionId:string,updates:update[]} or {sessionId:string,update:update}", {
    variants: ["move {operation,entity,x,y}", "owner {operation,entity,owner}", "resources {operation,playerId,steel,oil}", "research {operation,playerId,upgrade,completed?}", "godMode {operation,playerId,enabled?}"],
    sceneMutation: true,
    defaults: ["research.completed=true", "godMode.enabled=true"], bounds: ["1-400 updates", "legacy singular update accepts exactly one update"],
    example: { sessionId: "<lab-session-id>", updates: [{ operation: "move", entity: "subject", x: 1100, y: 960 }] },
  }),
  remove: descriptor("Atomically remove a bounded set of referenced entities.", "{sessionId:string,refs:(alias|u32)[]}", {
    sceneMutation: true,
    bounds: ["1-400 unique resolved references"], example: { sessionId: "<lab-session-id>", refs: ["subject"] },
  }),
  order: descriptor("Issue one normal authoritative game command as a selected player.", "{sessionId:string,playerId:u32,command:game-command,ignoreCommandLimits?:boolean}", {
    sceneMutation: true,
    variants: ["move|attackMove|attack|deconstruct|setupAntiTankGuns|tearDownAntiTankGuns|charge|useAbility|recastAbility|setAutocast|gather|build|train|adjustProductionRepeat|research|cancel|stop|holdPosition|setRally", "adjustProductionRepeat {c,buildings,unit,delta:-1|1} changes auto-build on one of 1-100 producers", "success reports authoritative enqueue admission and queuedAtTick, not effect completion"],
    defaults: ["ignoreCommandLimits=false", "queued omitted unless supplied by the command variant"], bounds: ["1-100 entity references", "command bridge JSON <=16 KiB"],
    example: { sessionId: "<lab-session-id>", playerId: 1, command: { c: "move", units: ["subject"], x: 1100, y: 960 } },
  }),
  time: descriptor("Pause, resume, speed, step, or seek authoritative room time.", "{sessionId:string,control:time-control}", {
    sceneMutation: true,
    variants: ["pause {action}", "resume {action,speed?}", "speed {action,speed}", "step {action,ticks?}", "seek {action,tick}"],
    defaults: ["resume.speed=1", "step.ticks=1"], bounds: ["speed 0-16 (resume >0)", "step 1-100 ticks", "seek tick 0-1000000"],
    example: { sessionId: "<lab-session-id>", control: { action: "step", ticks: 3 } },
  }),
  inspect: descriptor("Inspect bounded authoritative entity, player, room, and semantic camera/viewport/bounds facts.", "{sessionId:string,refs?:(alias|u32)[],kinds?:token[],owners?:u32[],cameraViewport?:boolean,limit?:int}", {
    defaults: ["refs/kinds/owners=unfiltered", "cameraViewport=false", "limit=25"], bounds: ["0-400 refs", "0-32 kinds", "0-16 owners", "limit 1-400"],
    example: { sessionId: "<lab-session-id>", refs: ["subject"], limit: 1 },
  }),
  camera: descriptor("Set the camera or focus a bounded referenced subject set; returns semantic camera/viewport/bounds facts.", "{sessionId:string,camera:camera-command}", {
    variants: ["focus {action,refs,padding?}", "set {action,snapshot:CameraSnapshotV1}"], defaults: ["focus.padding=32 for one unit, otherwise 48"],
    bounds: ["focus 1-400 refs", "padding 0-1024", "snapshot framingScale >0 and <=16"],
    example: { sessionId: "<lab-session-id>", camera: { action: "focus", refs: ["subject"] } },
  }),
  screenshot: descriptor("Capture a readiness-checked selected-renderer PNG and return its shareable Tailnet Preview URL.", "{sessionId:string,name?:token,presentation?:\"clean\"|\"normal\",viewport?:viewport,subjects?:(alias|u32)[]}", {
    variants: ["presentation=clean hides UI chrome", "presentation=normal retains visible Lab panels and game UI", "response.preview.url is the user-delivery URL; local capture paths are withheld"],
    defaults: ["name=scene", "presentation=clean", "viewport=current viewport", "subjects=[]"], bounds: ["0-400 subjects", "name 1-48 safe-token characters", "capture viewport 320-2048 x 240-2048", "24 detailed subject summaries"],
    example: { sessionId: "<lab-session-id>", name: "subject", presentation: "clean", subjects: ["subject"] },
  }),
  status: descriptor("Inspect daemon/session state; remains available across checkout mismatch.", "{sessionId?:string}", {
    scope: "daemon", lane: "observation", recordable: false,
    variants: ["without sessionId returns daemon service state", "with sessionId returns authoritative session, recorder, capture, and aliases"],
    defaults: ["sessionId omitted"], bounds: ["at most 400 returned aliases"], example: {},
  }),
  shutdown: descriptor("Stop the daemon and discard any active ephemeral session.", "{}", {
    scope: "daemon", lane: "lifecycle", timeoutClass: "lifecycle-media", recordable: false,
    bounds: ["no input fields"], example: {},
  }),
  export: descriptor("Export the current setup or replay to a confined portable artifact.", "{sessionId:string,kind:\"setup\"|\"replay\",name?:string,reproduction?:boolean}", {
    defaults: ["name=empty", "reproduction=false"], bounds: ["setup name <=80 UTF-8 bytes", "replay name <=120 UTF-8 bytes", "artifact <=8 MiB", "alias sidecar <=64 KiB and 400 aliases"],
    example: { sessionId: "<lab-session-id>", kind: "setup", name: "two-unit-scene", reproduction: true },
  }),
  import: descriptor("Destructively replace the ephemeral session from one confined artifact.", "{sessionId:string,kind:\"setup\"|\"replay\",artifactId?:string,path?:string,details?:boolean}", {
    variants: ["provide exactly one of artifactId or path", "details=false summarizes restored/stale aliases", "details=true returns every alias row and the raw import result"], defaults: ["details=false"], bounds: ["path <=1024 characters and beneath target/interact/lab", "artifact <=8 MiB", "at most 400 sidecar aliases", "12 default details per alias category"],
    example: { sessionId: "<lab-session-id>", kind: "setup", artifactId: "artifact_<32-hex>" },
  }),
  "artifact-inspect": descriptor("Inspect bounded metadata for one confined setup or replay artifact.", "{sessionId:string,kind?:\"setup\"|\"replay\",artifactId?:string,path?:string}", {
    variants: ["provide exactly one of artifactId or path", "kind may be omitted when artifactId/path identifies it"], bounds: ["artifact <=8 MiB", "at most 400 sidecar aliases"],
    example: { sessionId: "<lab-session-id>", artifactId: "artifact_<32-hex>" },
  }),
  "record-start": descriptor("Start one real-time clean-presentation H.264 recording.", "{sessionId:string,name?:token,maxDurationMs?:int,viewport?:viewport,crop?:{x,y,width,height},scale?:number,resumeSpeed?:number}", {
    recordable: false,
    variants: ["resumeSpeed atomically resumes authoritative time after the recorder has its initial frame"],
    defaults: ["name=recording", "maxDurationMs=10000", "viewport=current", "crop=game viewport", "scale=1", "resumeSpeed=omitted"], bounds: ["duration 1000-60000 ms", "viewport/crop <=2048", "scale 0.25-1", "resumeSpeed 0.01-16", "one active recorder", "64 MiB output"],
    example: { sessionId: "<lab-session-id>", name: "motion", maxDurationMs: 10000, resumeSpeed: 1 },
  }),
  "record-stop": descriptor("Finalize the active real-time recording and return shareable Tailnet video/contact-sheet previews.", "{sessionId:string}", {
    timeoutClass: "lifecycle-media", recordable: false,
    bounds: ["one active recorder", "six retained representative frames", "local artifact paths are withheld", "40 detailed aliases in the manifest"], example: { sessionId: "<lab-session-id>" },
  }),
  "record-wait": descriptor("Wait for the current recording to finalize without blocking other session commands.", "{sessionId:string}", {
    lane: "observation", timeoutClass: "lifecycle-media", recordable: false,
    variants: ["active and finalizing recordings share one completion", "a completed current recording returns its last result", "completion includes the same Tailnet preview URLs as record-stop"],
    bounds: ["a recording must have been started in the current session", "recording-only IPC timeout is capped at 420 seconds"],
    example: { sessionId: "<lab-session-id>" },
  }),
  "capture-fixed": descriptor("Capture a deterministic-environment fixed-step H.264 sequence and return a Tailnet video preview.", "{sessionId:string,name?:token,fps?:int,frameCount?:int,viewport?:viewport}", {
    timeoutClass: "lifecycle-media",
    defaults: ["name=fixed", "fps=30", "frameCount=30", "viewport=current"], bounds: ["paused room required", "fps 10-60", "1-1800 frames", "six retained representative PNGs", "Tailnet video/contact-sheet preview", "per-frame details in the manifest", "40 detailed aliases in the manifest"],
    example: { sessionId: "<lab-session-id>", name: "motion-fixed", fps: 30, frameCount: 60 },
  }),
  "capture-cancel": descriptor("Request cancellation of the active fixed-step capture.", "{sessionId:string}", {
    lane: "cancellation", recordable: false,
    bounds: ["an active fixed capture is required"], example: { sessionId: "<lab-session-id>" },
  }),
  "game-open": descriptor("Open or recover one isolated normal human-vs-AI match.", "{workspaceRoot?:string,map?:string,opponent?:\"ai_2_1\"|\"ai_turtle\",renderer?:\"pixi\"|\"babylon\",viewport?:viewport}", {
    scope: "daemon", lane: "lifecycle", timeoutClass: "startup", recordable: false,
    defaults: ["workspaceRoot=current worktree", "map=Default", "opponent=ai_2_1", "renderer=pixi", "viewport=1440x900 at DPR 1"],
    bounds: ["one session across Lab and game", "one local player and one AI opponent", "map <=64 UTF-8 bytes", "viewport 320-4096 x 240-4096"],
    example: { opponent: "ai_2_1", viewport: { width: 1200, height: 800, deviceScaleFactor: 1 } },
  }),
  "game-inspect": descriptor("Inspect the isolated match's bounded fog-filtered entities, player state, camera, and semantic UI.", "{sessionId:string,ids?:u32[],kinds?:token[],ownership?:\"owned\"|\"visible\",cameraViewport?:boolean,limit?:int}", {
    lane: "observation",
    defaults: ["ids/kinds=unfiltered", "ownership=owned", "cameraViewport=false", "limit=25"],
    bounds: ["0-400 unique ids", "0-32 kinds", "limit 1-400", "only the normal recipient's fog-filtered snapshot is inspectable"],
    example: { sessionId: "<game-session-id>", ownership: "owned", limit: 25 },
  }),
  "game-move": descriptor("Issue one normal move order for bounded locally owned units.", "{sessionId:string,units:u32[],x:number,y:number,queued?:boolean}", {
    sceneMutation: true,
    defaults: ["queued=false"],
    bounds: ["1-100 unique unit ids", "owned units only", "destination must be inside the map", "no attack, build, economy, ability, or arbitrary protocol commands"],
    example: { sessionId: "<game-session-id>", units: [42], x: 1100, y: 960 },
  }),
  "game-give-up": descriptor("Surrender the isolated match through the normal player give-up flow.", "{sessionId:string}", {
    sceneMutation: true,
    bounds: ["active isolated match only", "waits for the authoritative score screen"],
    example: { sessionId: "<game-session-id>" },
  }),
  "game-camera": descriptor("Set the camera or focus bounded visible entity ids.", "{sessionId:string,camera:game-camera-command}", {
    variants: ["focus {action,entities,padding?}", "set {action,snapshot:CameraSnapshotV1}"],
    defaults: ["focus.padding=32 for one unit, otherwise 48"],
    bounds: ["focus 1-400 unique ids", "padding 0-1024", "snapshot framingScale >0 and <=16"],
    example: { sessionId: "<game-session-id>", camera: { action: "focus", entities: [42] } },
  }),
  "game-screenshot": descriptor("Capture a readiness-checked match PNG with UI visible by default.", "{sessionId:string,name?:token,presentation?:\"normal\"|\"clean\",viewport?:viewport,subjects?:u32[]}", {
    variants: ["presentation=normal retains the HUD and overlays", "presentation=clean captures only the rendered battlefield", "response.preview.url is the user-delivery URL"],
    defaults: ["name=game", "presentation=normal", "viewport=current", "subjects=[]"],
    bounds: ["0-400 unique subject ids", "capture viewport 320-2048 x 240-2048", "24 detailed subject summaries"],
    example: { sessionId: "<game-session-id>", name: "opening-ui", presentation: "normal" },
  }),
  "game-record-start": descriptor("Start one real-time H.264 recording with match UI visible by default.", "{sessionId:string,name?:token,maxDurationMs?:int,viewport?:viewport,crop?:crop,scale?:number,presentation?:\"normal\"|\"clean\"}", {
    recordable: false,
    variants: ["presentation=normal retains the HUD and overlays", "presentation=clean records only the battlefield"],
    defaults: ["name=game", "maxDurationMs=10000", "viewport=current", "crop=game viewport", "scale=1", "presentation=normal"],
    bounds: ["duration 1000-60000 ms", "viewport/crop <=2048", "scale 0.25-1", "one active recorder", "64 MiB output"],
    example: { sessionId: "<game-session-id>", name: "opening-move", maxDurationMs: 10000, presentation: "normal" },
  }),
});

export const INTERACT_COMMAND_REGISTRY: Readonly<Record<string, CommandDefinition>> = Object.freeze(Object.fromEntries(
  Object.entries(COMMAND_RECORDS).map(([name, record]) => [name, Object.freeze({
    name,
    scope: record.scope,
    lane: record.lane,
    timeoutClass: record.timeoutClass,
    validator: validatorFor(name),
    handlerKey: name,
    sceneMutation: record.sceneMutation,
    recordable: record.recordable,
    help: record.help,
  })]),
));

export const INTERACT_COMMAND_KEYS = Object.freeze(Object.keys(INTERACT_COMMAND_REGISTRY));

const NAMESPACE_COMMAND_KEYS = Object.freeze({
  lab: Object.freeze(Object.fromEntries(INTERACT_COMMAND_KEYS.filter((name) => !name.startsWith("game-")).map((name) => [name, name]))),
  game: Object.freeze({
    open: "game-open",
    close: "close",
    status: "status",
    inspect: "game-inspect",
    move: "game-move",
    camera: "game-camera",
    screenshot: "game-screenshot",
    "record-start": "game-record-start",
    "record-stop": "record-stop",
    "record-wait": "record-wait",
    "give-up": "game-give-up",
    shutdown: "shutdown",
  }),
});

export const INTERACT_NAMESPACES = Object.freeze(Object.fromEntries(
  Object.entries(NAMESPACE_COMMAND_KEYS).map(([namespace, commands]) => [namespace, Object.freeze(Object.keys(commands))]),
));

// Backward-compatible name for the original public Lab command catalog.
export const INTERACT_COMMANDS = INTERACT_NAMESPACES.lab;

export function namespaceCommandKey(namespace: string, command: string): string | null {
  return (NAMESPACE_COMMAND_KEYS as Record<string, Readonly<Record<string, string>>>)[namespace]?.[command] || null;
}

export function namespaceCommandDefinition(namespace: string, command: string): CommandDefinition | null {
  const key = namespaceCommandKey(namespace, command);
  return key ? commandDefinition(key) : null;
}

export function commandDefinition(command: string): CommandDefinition | null {
  return INTERACT_COMMAND_REGISTRY[command] || null;
}

export function validateCommandInput(command: string, input: unknown) {
  const definition = commandDefinition(command);
  if (!definition) throw Object.assign(new Error(`Unknown command ${JSON.stringify(command)}.`), { code: "unknownCommand" });
  return definition.validator(input);
}

export function requestTimeoutMs(command: string) {
  const definition = commandDefinition(command);
  if (definition?.timeoutClass === "startup") return STARTUP_REQUEST_TIMEOUT_MS;
  return definition?.timeoutClass === "lifecycle-media" ? RECORDING_REQUEST_TIMEOUT_MS : REQUEST_TIMEOUT_MS;
}
