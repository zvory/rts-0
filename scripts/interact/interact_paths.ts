import fs from "node:fs";
import path from "node:path";

export function interactArtifactRoot(mode: "lab" | "game" | "scenario") {
  return path.join("target", "interact", mode);
}

export function createInteractSessionDirectory(workspaceRoot: string, map: unknown, mode: "lab" | "game" | "scenario") {
  const root = path.join(workspaceRoot, interactArtifactRoot(mode), "sessions");
  fs.mkdirSync(root, { recursive: true });
  const token = String(map || "default").replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 32) || "default";
  const name = `${token}-${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
  const directory = path.join(root, name);
  fs.mkdirSync(directory, { recursive: true });
  return directory;
}
