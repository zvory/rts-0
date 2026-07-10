import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import { pathToFileURL } from "node:url";

import { LabInteractService, conciseError, loadDriverFactory, normalizeError, validateCommandInput } from "./command_service.mjs";
import {
  IPC_VERSION, MAX_REQUEST_BYTES, REQUEST_TIMEOUT_MS, cleanupOwnedRuntime, configuredIdleMs,
  prepareRuntime, runtimePaths, writeState,
} from "./runtime.mjs";

export async function runDaemon({ workspaceRoot = process.cwd(), idleMs = configuredIdleMs() } = {}) {
  const paths = runtimePaths(workspaceRoot);
  prepareRuntime(paths);
  const driverFactory = await loadDriverFactory(paths.workspaceRoot);
  const service = new LabInteractService({ workspaceRoot: paths.workspaceRoot, driverFactory });
  let activeRequests = 0;
  let lastInteractionAt = Date.now();
  let idleTimer = null;
  let stopping = false;
  let shutdownRequested = false;
  const sockets = new Set();
  const daemonId = crypto.randomUUID();
  const capability = crypto.randomBytes(32).toString("hex");

  const state = () => ({
    pid: process.pid,
    protocolVersion: IPC_VERSION,
    daemonId,
    capability,
    workspaceRoot: paths.workspaceRoot,
    socket: paths.socket,
    idleMs,
    startedAt: startedAt,
    lastInteractionAt,
    activeRequests,
  });
  const startedAt = new Date().toISOString();

  const cleanup = () => cleanupOwnedRuntime(paths, daemonId);
  const shutdown = async (reason) => {
    if (stopping) return;
    stopping = true;
    clearTimeout(idleTimer);
    for (const socket of sockets) socket.destroy();
    server.closeAllConnections?.();
    await new Promise((resolve) => server.close(resolve));
    await service.shutdown(reason);
    cleanup();
  };
  const scheduleIdle = () => {
    clearTimeout(idleTimer);
    if (stopping || activeRequests > 0) return;
    const remaining = Math.max(1, lastInteractionAt + idleMs - Date.now());
    idleTimer = setTimeout(() => { void shutdown("idleTimeout"); }, remaining);
  };

  const server = net.createServer((socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    socket.setEncoding("utf8");
    socket.setTimeout(5_000, () => socket.destroy());
    let body = "";
    let handled = false;
    socket.on("data", (chunk) => {
      body += chunk;
      if (Buffer.byteLength(body) > MAX_REQUEST_BYTES) {
        handled = true;
        socket.end(`${JSON.stringify(errorEnvelope("requestTooLarge", "Request exceeds 1 MiB."))}\n`);
        socket.destroySoon?.();
      } else if (body.includes("\n") && !handled) {
        handled = true;
        void handle(body.slice(0, body.indexOf("\n")), socket);
      }
    });
    socket.on("error", () => {});
  });

  const handle = async (line, socket) => {
    let command = "";
    let admitted = false;
    try {
      const request = JSON.parse(line);
      if (!request || typeof request !== "object" || Array.isArray(request)) throw Object.assign(new Error("Request must be a JSON object."), { code: "invalidRequest" });
      if (Object.keys(request).length === 2 && request.protocolVersion === IPC_VERSION && request.probe === "lab-interact") {
        socket.end(`${JSON.stringify({ ok: true, probe: { protocolVersion: IPC_VERSION, daemonId, pid: process.pid, workspaceRoot: paths.workspaceRoot } })}\n`);
        return;
      }
      const keys = Object.keys(request);
      if (keys.some((key) => !["protocolVersion", "daemonId", "capability", "command", "input"].includes(key)) ||
          request.protocolVersion !== IPC_VERSION || request.daemonId !== daemonId || request.capability !== capability ||
          typeof request.command !== "string" || !request.input || typeof request.input !== "object" || Array.isArray(request.input)) {
        throw Object.assign(new Error("Request identity, version, or command envelope is invalid."), { code: "invalidRequest" });
      }
      command = request.command;
      validateCommandInput(command, request.input);
      if (shutdownRequested) throw Object.assign(new Error("Lab Interact is already shutting down."), { code: "serviceClosed" });
      if (command === "shutdown") shutdownRequested = true;
      socket.setTimeout(REQUEST_TIMEOUT_MS + 5_000, () => socket.destroy());
      activeRequests += 1;
      admitted = true;
      lastInteractionAt = Date.now();
      clearTimeout(idleTimer);
      writeState(paths, state());
      const result = await service.execute(command, request.input);
      socket.end(`${JSON.stringify({ ok: true, result })}\n`);
    } catch (error) {
      const normalized = normalizeError(error);
      socket.end(`${JSON.stringify(errorEnvelope(normalized.code, normalized.message))}\n`);
    } finally {
      if (admitted) activeRequests -= 1;
      if (admitted) lastInteractionAt = Date.now();
      if (shutdownRequested && activeRequests === 0) {
        socket.once("close", () => { void shutdown("explicit"); });
      } else if (admitted && !shutdownRequested) {
        writeState(paths, state());
        scheduleIdle();
      }
    }
  };

  server.on("error", async (error) => {
    await service.shutdown("socketError").catch(() => {});
    cleanup();
    process.stderr.write(`${JSON.stringify(errorEnvelope(error.code || "socketError", conciseError(error)))}\n`);
    process.exitCode = 1;
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(paths.socket, () => {
      server.removeListener("error", reject);
      resolve();
    });
  });
  fs.chmodSync(paths.socket, 0o600);
  const startupDelayMs = Number(process.env.RTS_LAB_INTERACT_TEST_STARTUP_DELAY_MS || 0);
  if (Number.isInteger(startupDelayMs) && startupDelayMs > 0 && startupDelayMs <= 2_000) {
    await new Promise((resolve) => setTimeout(resolve, startupDelayMs));
  }
  writeState(paths, state());
  fs.rmSync(paths.lock, { force: true });
  scheduleIdle();
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    process.once(signal, () => { void shutdown(signal); });
  }
  process.once("exit", cleanup);
  return { server, service, paths, shutdown };
}

function errorEnvelope(code, message) {
  return { ok: false, error: { code: code || "commandFailed", message: String(message).slice(0, 1000) } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const workspaceRoot = process.argv[2];
  runDaemon({ workspaceRoot }).catch((error) => {
    process.stderr.write(`${JSON.stringify(errorEnvelope(error.code || "startupFailed", conciseError(error)))}\n`);
    process.exitCode = 1;
  });
}
