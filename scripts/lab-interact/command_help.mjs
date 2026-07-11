import { LAB_INTERACT_COMMANDS } from "./command_service.mjs";

function descriptor(summary, acceptedShape, {
  variants = [], defaults = [], bounds = [], example,
} = {}) {
  return Object.freeze({ summary, acceptedShape, variants, defaults, bounds, example });
}

export const LAB_INTERACT_COMMAND_HELP = Object.freeze({
  open: descriptor(
    "Open or recover the one authoritative Lab Interact session for this worktree.",
    "{workspaceRoot?:string,map?:token,seed?:string|u32,scenario?:token,viewport?:{width:int,height:int,deviceScaleFactor?:number}}",
    {
      defaults: ["workspaceRoot=current worktree", "map=Default", "scenario=blank", "seed=empty", "viewport=1440x900 at DPR 1"],
      bounds: ["one session", "map/scenario <=48 safe-token characters", "viewport 320-4096 x 240-4096", "DPR >0 and <=4"],
      example: { viewport: { width: 1000, height: 700, deviceScaleFactor: 1 } },
    },
  ),
  close: descriptor("Close the active browser/server session without stopping the daemon.", "{sessionId:string}", {
    bounds: ["sessionId must be the current Lab Interact session id"], example: { sessionId: "<lab-session-id>" },
  }),
  reset: descriptor("Reset the ephemeral Lab scene and reconcile aliases that still match.", "{sessionId:string}", {
    bounds: ["sessionId must be current", "at most 400 aliases are reconciled"], example: { sessionId: "<lab-session-id>" },
  }),
  catalog: descriptor("List bounded maps, players, factions, kinds, upgrades, commands, or abilities.", "{sessionId:string,categories?:category[]}", {
    variants: ["category=maps|players|factions|units|buildings|upgrades|commands|abilities"],
    defaults: ["categories=all categories"], bounds: ["0-8 unique requested categories"],
    example: { sessionId: "<lab-session-id>", categories: ["players", "units", "commands"] },
  }),
  spawn: descriptor("Atomically spawn one bounded batch and optionally assign aliases.", "{sessionId:string,spawns:[{owner:u32,kind:token,x:number,y:number,completed?:boolean,alias?:alias}],details?:boolean}", {
    variants: ["details=false returns counts and at most 12 alias/id rows", "details=true returns every decorated entity and raw authoritative outcome"],
    defaults: ["completed=true", "alias=none", "details=false"], bounds: ["1-400 spawns", "400 aliases per session", "alias [A-Za-z][A-Za-z0-9_-]{0,31}", "12 default response details"],
    example: { sessionId: "<lab-session-id>", spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "subject" }] },
  }),
  update: descriptor("Atomically apply a bounded batch of Lab scene updates.", "{sessionId:string,updates:update[]} or {sessionId:string,update:update}", {
    variants: ["move {operation,entity,x,y}", "owner {operation,entity,owner}", "resources {operation,playerId,steel,oil}", "research {operation,playerId,upgrade,completed?}", "godMode {operation,playerId,enabled?}"],
    defaults: ["research.completed=true", "godMode.enabled=true"], bounds: ["1-400 updates", "legacy singular update accepts exactly one update"],
    example: { sessionId: "<lab-session-id>", updates: [{ operation: "move", entity: "subject", x: 1100, y: 960 }] },
  }),
  remove: descriptor("Atomically remove a bounded set of referenced entities.", "{sessionId:string,refs:(alias|u32)[]}", {
    bounds: ["1-400 unique resolved references"], example: { sessionId: "<lab-session-id>", refs: ["subject"] },
  }),
  order: descriptor("Issue one normal authoritative game command as a selected player.", "{sessionId:string,playerId:u32,command:game-command,ignoreCommandLimits?:boolean}", {
    variants: ["move|attackMove|attack|deconstruct|setupAntiTankGuns|tearDownAntiTankGuns|charge|useAbility|recastAbility|setAutocast|gather|build|train|research|cancel|stop|holdPosition|setRally"],
    defaults: ["ignoreCommandLimits=false", "queued omitted unless supplied by the command variant"], bounds: ["1-100 unit references", "command bridge JSON <=16 KiB"],
    example: { sessionId: "<lab-session-id>", playerId: 1, command: { c: "move", units: ["subject"], x: 1100, y: 960 } },
  }),
  time: descriptor("Pause, resume, speed, step, or seek authoritative room time.", "{sessionId:string,control:time-control}", {
    variants: ["pause {action}", "resume {action,speed?}", "speed {action,speed}", "step {action,ticks?}", "seek {action,tick}"],
    defaults: ["resume.speed=1", "step.ticks=1"], bounds: ["speed 0-16 (resume >0)", "step 1-100 ticks", "seek tick 0-1000000"],
    example: { sessionId: "<lab-session-id>", control: { action: "step", ticks: 3 } },
  }),
  inspect: descriptor("Inspect bounded authoritative entity, player, room, and camera facts.", "{sessionId:string,refs?:(alias|u32)[],kinds?:token[],owners?:u32[],cameraViewport?:boolean,limit?:int}", {
    defaults: ["refs/kinds/owners=unfiltered", "cameraViewport=false", "limit=25"], bounds: ["0-400 refs", "0-32 kinds", "0-16 owners", "limit 1-400"],
    example: { sessionId: "<lab-session-id>", refs: ["subject"], limit: 1 },
  }),
  camera: descriptor("Set the camera or focus a bounded referenced subject set.", "{sessionId:string,camera:camera-command}", {
    variants: ["focus {action,refs,padding?}", "set {action,centerX?,centerY?,zoom?}"], defaults: ["focus.padding=32 for one unit, otherwise 48"],
    bounds: ["focus 1-400 refs", "padding 0-1024", "zoom >0 and <=16", "centerX and centerY supplied together"],
    example: { sessionId: "<lab-session-id>", camera: { action: "focus", refs: ["subject"] } },
  }),
  screenshot: descriptor("Capture a readiness-checked Pixi PNG and bounded adjacent manifest.", "{sessionId:string,name?:token,presentation?:\"clean\"|\"normal\",viewport?:viewport,subjects?:(alias|u32)[]}", {
    variants: ["presentation=clean hides UI chrome", "presentation=normal retains visible Lab panels and game UI"],
    defaults: ["name=scene", "presentation=clean", "viewport=current viewport", "subjects=[]"], bounds: ["0-400 subjects", "name 1-48 safe-token characters", "capture viewport 320-2048 x 240-2048", "24 detailed subject summaries"],
    example: { sessionId: "<lab-session-id>", name: "subject", presentation: "clean", subjects: ["subject"] },
  }),
  status: descriptor("Inspect daemon/session state; remains available across checkout mismatch.", "{sessionId?:string}", {
    variants: ["without sessionId returns daemon service state", "with sessionId returns authoritative session, recorder, capture, and aliases"],
    defaults: ["sessionId omitted"], bounds: ["at most 400 returned aliases"], example: {},
  }),
  shutdown: descriptor("Stop the daemon and discard any active ephemeral session.", "{}", {
    bounds: ["no input fields"], example: {},
  }),
  export: descriptor("Export the current setup or replay to a confined portable artifact.", "{sessionId:string,kind:\"setup\"|\"replay\",name?:string,reproduction?:boolean}", {
    defaults: ["name=empty", "reproduction=false"], bounds: ["setup name <=80 UTF-8 bytes", "replay name <=120 UTF-8 bytes", "artifact <=8 MiB", "alias sidecar <=64 KiB and 400 aliases"],
    example: { sessionId: "<lab-session-id>", kind: "setup", name: "two-unit-scene", reproduction: true },
  }),
  import: descriptor("Destructively replace the ephemeral session from one confined artifact.", "{sessionId:string,kind:\"setup\"|\"replay\",artifactId?:string,path?:string,details?:boolean}", {
    variants: ["provide exactly one of artifactId or path", "details=false summarizes restored/stale aliases", "details=true returns every alias row and the raw import result"], defaults: ["details=false"], bounds: ["path <=1024 characters and beneath target/lab-interact", "artifact <=8 MiB", "at most 400 sidecar aliases", "12 default details per alias category"],
    example: { sessionId: "<lab-session-id>", kind: "setup", artifactId: "artifact_<32-hex>" },
  }),
  "artifact-inspect": descriptor("Inspect bounded metadata for one confined setup or replay artifact.", "{sessionId:string,kind?:\"setup\"|\"replay\",artifactId?:string,path?:string}", {
    variants: ["provide exactly one of artifactId or path", "kind may be omitted when artifactId/path identifies it"], bounds: ["artifact <=8 MiB", "at most 400 sidecar aliases"],
    example: { sessionId: "<lab-session-id>", artifactId: "artifact_<32-hex>" },
  }),
  "record-start": descriptor("Start one real-time clean-presentation H.264 recording.", "{sessionId:string,name?:token,maxDurationMs?:int,viewport?:viewport,crop?:{x,y,width,height},scale?:number,resumeSpeed?:number}", {
    variants: ["resumeSpeed atomically resumes authoritative time after the recorder has its initial frame"],
    defaults: ["name=recording", "maxDurationMs=10000", "viewport=current", "crop=game viewport", "scale=1", "resumeSpeed=omitted"], bounds: ["duration 1000-60000 ms", "viewport/crop <=2048", "scale 0.25-1", "resumeSpeed 0.01-16", "one active recorder", "64 MiB output"],
    example: { sessionId: "<lab-session-id>", name: "motion", maxDurationMs: 10000, resumeSpeed: 1 },
  }),
  "record-stop": descriptor("Finalize the active real-time recording and return confined artifact paths.", "{sessionId:string}", {
    bounds: ["one active recorder", "six representative frame paths", "40 detailed aliases in the manifest"], example: { sessionId: "<lab-session-id>" },
  }),
  "record-wait": descriptor("Wait for the current recording to finalize without blocking other session commands.", "{sessionId:string}", {
    variants: ["active and finalizing recordings share one completion", "a completed current recording returns its last result"],
    bounds: ["a recording must have been started in the current session", "recording-only IPC timeout is capped at 420 seconds"],
    example: { sessionId: "<lab-session-id>" },
  }),
  "capture-fixed": descriptor("Capture a deterministic-environment fixed-step H.264 sequence.", "{sessionId:string,name?:token,fps?:int,frameCount?:int,viewport?:viewport}", {
    defaults: ["name=fixed", "fps=30", "frameCount=30", "viewport=current"], bounds: ["paused room required", "fps 10-60", "1-180 frames", "40 detailed aliases in the manifest"],
    example: { sessionId: "<lab-session-id>", name: "motion-fixed", fps: 30, frameCount: 60 },
  }),
  "capture-cancel": descriptor("Request cancellation of the active fixed-step capture.", "{sessionId:string}", {
    bounds: ["an active fixed capture is required"], example: { sessionId: "<lab-session-id>" },
  }),
});

export function commandHelp(command) {
  return LAB_INTERACT_COMMAND_HELP[command] || null;
}

export function helpCatalog() {
  return LAB_INTERACT_COMMANDS.map((command) => ({
    command,
    summary: LAB_INTERACT_COMMAND_HELP[command].summary,
  }));
}

const coverage = Object.keys(LAB_INTERACT_COMMAND_HELP).sort();
const catalog = [...LAB_INTERACT_COMMANDS].sort();
if (coverage.length !== catalog.length || coverage.some((command, index) => command !== catalog[index])) {
  throw new Error(`Lab Interact help coverage mismatch: commands=${catalog.join(",")} descriptors=${coverage.join(",")}`);
}
