import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export class WorkspaceInspectionError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "WorkspaceInspectionError";
    this.code = code;
  }
}

export function validateWorkspaceRoot(workspaceRoot) {
  if (!workspaceRoot) throw new WorkspaceInspectionError("workspaceRequired", "workspaceRoot is required.");
  let root;
  try { root = fs.realpathSync(workspaceRoot); } catch {
    throw new WorkspaceInspectionError("invalidWorkspace", `Workspace does not exist: ${workspaceRoot}`);
  }
  if (!fs.existsSync(path.join(root, "server", "Cargo.toml")) || !fs.existsSync(path.join(root, "client", "src", "main.js"))) {
    throw new WorkspaceInspectionError("invalidWorkspace", "workspaceRoot is not a Bewegungskrieg checkout.");
  }
  const topLevel = git(root, ["rev-parse", "--show-toplevel"]);
  if (!topLevel || fs.realpathSync(topLevel) !== root) {
    throw new WorkspaceInspectionError("invalidWorkspace", "workspaceRoot must be the Git checkout top level.");
  }
  const head = git(root, ["rev-parse", "HEAD"]);
  if (!/^[0-9a-f]{40}$/i.test(head || "")) {
    throw new WorkspaceInspectionError("invalidWorkspace", "workspaceRoot has no valid Git HEAD.");
  }
  return { root, branch: git(root, ["branch", "--show-current"]) || "HEAD", head };
}

export function findChrome(explicit, env = process.env) {
  const candidates = [
    explicit,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ...pathCandidates("google-chrome-stable", env),
    ...pathCandidates("google-chrome", env),
    ...pathCandidates("chromium-browser", env),
    ...pathCandidates("chromium", env),
  ].filter(Boolean);
  const chrome = candidates.find((candidate) => fs.existsSync(candidate));
  if (!chrome) throw new WorkspaceInspectionError("chromeUnavailable", "Chrome/Chromium not found; set CHROME=/path/to/chrome.");
  return chrome;
}

function git(cwd, args) {
  const result = spawnSync("git", ["-C", cwd, ...args], {
    encoding: "utf8",
    timeout: 2_000,
    maxBuffer: 64 * 1024,
  });
  return result.status === 0 ? result.stdout.trim() : "";
}

function pathCandidates(command, env) {
  return String(env.PATH || "").split(path.delimiter).filter(Boolean).map((directory) => path.join(directory, command));
}
