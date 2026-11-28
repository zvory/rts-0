// Real authoritative movement and Pixi capture, not a screenshot composite.
import { runCli } from "./interact/cli.ts";

const command = async (namespace, name, input = {}) => {
  const response = await runCli([namespace, name, JSON.stringify(input)]);
  if (!response.ok) throw new Error(JSON.stringify(response.error));
  return response.result;
};
let capture;
try {
  const { sessionId } = await command("lab", "open", {
    visualProfile: "rifleman-directional-prototype",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  await command("lab", "catalog", { sessionId, categories: ["players", "units", "buildings"] });
  await command("lab", "time", { sessionId, control: { action: "pause" } });
  await command("lab", "spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 1070, y: 450, alias: "directionalWalker" },
    { owner: 1, kind: "barracks", x: 944, y: 320, alias: "directionalBarracks" },
  ] });
  await command("lab", "camera", { sessionId, camera: { action: "set", snapshot: {
    version: 1, focus: { x: 1000, y: 450 }, framingScale: 2.2, boundsPolicy: "mapOverscroll",
  } } });
  await command("lab", "select", { sessionId, refs: [] });
  console.log(JSON.stringify({ still: await command("lab", "screenshot", {
    sessionId, name: "directional-rifleman-in-game", presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 }, subjects: ["directionalWalker"],
  }) }));
  // Keep the clock running very slowly during enqueueing: the bridge otherwise
  // steps paused time after each command, consuming the route before capture.
  await command("lab", "time", { sessionId, control: { action: "resume", speed: 0.01 } });
  // One sixteen-sided circuit. Every leg is a normal move intent; the server
  // owns positions, turn rate, collisions and queue promotion. No facing spoofing.
  for (let i = 1; i <= 16; i++) {
    const angle = i * Math.PI / 8;
    await command("lab", "order", { sessionId, playerId: 1, ignoreCommandLimits: true,
      command: { c: "move", units: ["directionalWalker"],
        x: 1000 + 70 * Math.cos(angle), y: 450 + 70 * Math.sin(angle), queued: i > 1 },
    });
  }
  await command("lab", "time", { sessionId, control: { action: "pause" } });
  console.log(JSON.stringify({ queuedScene: await command("lab", "inspect", {
    sessionId, refs: ["directionalWalker"],
  }) }));
  capture = await command("lab", "capture-fixed", {
    sessionId, name: "directional-rifleman-move-orders", fps: 30, frameCount: 360,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  console.log(JSON.stringify({ capture, finalScene: await command("lab", "inspect", {
    sessionId, refs: ["directionalWalker"],
  }) }));
} finally {
  await command("lab", "shutdown");
}
const video = await command("media", "export", {
  artifactId: capture.artifactId, format: "webm", preset: "quality",
});
console.log(JSON.stringify({ video: await command("media", "inspect", { artifactId: video.artifactId }),
  published: await command("media", "publish", { artifactId: video.artifactId }) }));
