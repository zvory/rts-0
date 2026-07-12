import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

import {
  LabInteractError, LabInteractService, conciseError, loadDriverFactory, normalizeError,
  validateCommandInput,
} from "./command_service.mjs";
import {
  IPC_VERSION, MAX_REQUEST_BYTES, claimStartupLock, cleanupOwnedRuntime,
  checkoutCommit, configuredIdleMs, prepareRuntime, removeOwnedStartupLock, runtimePaths, startupLockOwned, writeState,
  requestTimeoutMs, writeStartupError,
} from "./runtime.mjs";
import { LabInteractTailnetPreview } from "./tailnet_preview.mjs";

export async function runDaemon({ workspaceRoot = process.cwd(), idleMs = configuredIdleMs(), startupNonce = "" } = {}) {
  const paths = runtimePaths(workspaceRoot);
  prepareRuntime(paths);
  if (!/^[a-f0-9]{32}$/.test(startupNonce)) throw new Error("Lab Interact daemon requires a valid startup nonce.");
  claimStartupLock(paths, startupNonce);
  const prebindDelayMs = Number(process.env.RTS_LAB_INTERACT_TEST_PREBIND_DELAY_MS || 0);
  if (Number.isInteger(prebindDelayMs) && prebindDelayMs > 0 && prebindDelayMs <= 2_000) {
    await new Promise((resolve) => setTimeout(resolve, prebindDelayMs));
  }
  if (!startupLockOwned(paths, startupNonce, process.pid, "daemon")) {
    throw new Error("Lab Interact startup lease was replaced before the daemon could bind.");
  }
  const driverFactory = await loadDriverFactory(paths.workspaceRoot);
  let artifactPreview = null;
  let activeRequests = 0;
  let lastInteractionAt = Date.now();
  let lastInteractionMark = performance.now();
  let idleTimer = null;
  let stopping = false;
  let shutdownRequested = false;
  const sockets = new Set();
  const daemonId = crypto.randomUUID();
  const capability = crypto.randomBytes(32).toString("hex");
  const configuredCheckoutCommit = process.env.RTS_LAB_INTERACT_TEST_CHECKOUT_COMMIT || "";
  if (configuredCheckoutCommit && !/^[a-f0-9]{40}$/.test(configuredCheckoutCommit)) {
    throw new Error("RTS_LAB_INTERACT_TEST_CHECKOUT_COMMIT must be a lowercase 40-character Git SHA.");
  }
  const daemonCheckoutCommit = configuredCheckoutCommit || checkoutCommit(paths.workspaceRoot);
  const omitCheckoutMetadata = process.env.RTS_LAB_INTERACT_TEST_OMIT_CHECKOUT === "1";

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
    activeSessions: service.sessions.size,
    ...(!omitCheckoutMetadata ? { checkoutCommit: daemonCheckoutCommit } : {}),
  });
  const startedAt = new Date().toISOString();

  const cleanup = () => cleanupOwnedRuntime(paths, daemonId);
  const recordInteraction = () => {
    lastInteractionAt = Date.now();
    lastInteractionMark = performance.now();
  };
  artifactPreview = new LabInteractTailnetPreview({
    workspaceRoot: paths.workspaceRoot,
    onAccess: () => {
      if (stopping || shutdownRequested) return;
      recordInteraction();
      writeState(paths, state());
      scheduleIdle();
    },
  });
  const service = new LabInteractService({
    workspaceRoot: paths.workspaceRoot,
    driverFactory,
    artifactPreview,
  });
  const shutdown = async (reason) => {
    if (stopping) return;
    stopping = true;
    clearTimeout(idleTimer);
    for (const socket of sockets) socket.destroy();
    server.closeAllConnections?.();
    await new Promise((resolve) => server.close(resolve));
    await service.shutdown(reason);
    await artifactPreview.close();
    cleanup();
  };
  const scheduleIdle = () => {
    clearTimeout(idleTimer);
    if (stopping || activeRequests > 0) return;
    const remaining = Math.max(1, lastInteractionMark + idleMs - performance.now());
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
        socket.end(`${JSON.stringify({ ok: true, probe: {
          protocolVersion: IPC_VERSION,
          daemonId,
          pid: process.pid,
          workspaceRoot: paths.workspaceRoot,
          ...(!omitCheckoutMetadata ? { checkoutCommit: daemonCheckoutCommit } : {}),
        } })}\n`);
        return;
      }
      const keys = Object.keys(request);
      if (keys.some((key) => !["protocolVersion", "daemonId", "capability", "command", "input", "refreshCheckout"].includes(key)) ||
          request.protocolVersion !== IPC_VERSION || request.daemonId !== daemonId || request.capability !== capability ||
          typeof request.command !== "string" || !request.input || typeof request.input !== "object" || Array.isArray(request.input)) {
        throw Object.assign(new Error("Request identity, version, or command envelope is invalid."), { code: "invalidRequest" });
      }
      command = request.command;
      validateCommandInput(command, request.input);
      if (shutdownRequested) throw Object.assign(new Error("Lab Interact is already shutting down."), { code: "serviceClosed" });
      if (request.refreshCheckout != null) {
        const requestedCommit = String(request.refreshCheckout || "").toLowerCase();
        if (command !== "shutdown" || !/^[a-f0-9]{40}$/.test(requestedCommit)) {
          throw Object.assign(new Error("Checkout refresh request is invalid."), { code: "invalidRequest" });
        }
        if (activeRequests !== 0 || !service.canRefreshCheckout()) {
          throw new LabInteractError(
            "daemonCheckoutMismatch",
            "The Lab Interact daemon belongs to another checkout commit and has active work. The scene was preserved; inspect status, then explicitly shut down when it is safe to discard.",
            {
              daemonCommit: daemonCheckoutCommit,
              checkoutCommit: requestedCommit,
              recoveryCommand: "node scripts/lab-interact/cli.mjs shutdown",
            },
          );
        }
      }
      if (command === "shutdown") shutdownRequested = true;
      socket.setTimeout(requestTimeoutMs(command) + 5_000, () => socket.destroy());
      activeRequests += 1;
      admitted = true;
      recordInteraction();
      clearTimeout(idleTimer);
      writeState(paths, state());
      const result = await service.execute(command, request.input);
      socket.end(`${JSON.stringify({ ok: true, result })}\n`);
    } catch (error) {
      const normalized = normalizeError(error);
      socket.end(`${JSON.stringify(errorEnvelope(normalized.code, normalized.message, normalized.details))}\n`);
    } finally {
      if (admitted) activeRequests -= 1;
      if (admitted) recordInteraction();
      if (shutdownRequested && activeRequests === 0) {
        socket.once("close", () => { void shutdown("explicit"); });
      } else if (admitted && !shutdownRequested) {
        writeState(paths, state());
        scheduleIdle();
      }
    }
  };

  const handleRuntimeError = async (error) => {
    await shutdown("socketError").catch(() => cleanup());
    process.stderr.write(`${JSON.stringify(errorEnvelope(error.code || "socketError", conciseError(error)))}\n`);
    process.exitCode = 1;
  };

  await new Promise((resolve, reject) => {
    if (!startupLockOwned(paths, startupNonce, process.pid, "daemon")) {
      reject(new Error("Lab Interact startup lease was replaced before socket bind."));
      return;
    }
    server.once("error", reject);
    server.listen(paths.socket, () => {
      server.removeListener("error", reject);
      resolve();
    });
  });
  try {
    fs.chmodSync(paths.socket, 0o600);
    const startupDelayMs = Number(process.env.RTS_LAB_INTERACT_TEST_STARTUP_DELAY_MS || 0);
    if (Number.isInteger(startupDelayMs) && startupDelayMs > 0 && startupDelayMs <= 2_000) {
      await new Promise((resolve) => setTimeout(resolve, startupDelayMs));
    }
    if (!startupLockOwned(paths, startupNonce, process.pid, "daemon")) {
      throw new Error("Lab Interact startup lease was replaced before state publication.");
    }
    writeState(paths, state());
    removeOwnedStartupLock(paths, startupNonce, process.pid, "daemon");
  } catch (error) {
    for (const socket of sockets) socket.destroy();
    await new Promise((resolve) => server.close(resolve));
    await service.shutdown("startupFailed");
    cleanup();
    throw error;
  }
  server.on("error", handleRuntimeError);
  scheduleIdle();
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    process.once(signal, () => { void shutdown(signal); });
  }
  process.once("exit", cleanup);
  return { server, service, paths, shutdown };
}

function errorEnvelope(code, message, details = undefined) {
  return {
    ok: false,
    error: {
      code: code || "commandFailed",
      message: String(message).slice(0, 1000),
      ...(details && typeof details === "object" && Object.keys(details).length ? { details } : {}),
    },
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const workspaceRoot = process.argv[2];
  const startupNonce = process.argv[3] || "";
  runDaemon({ workspaceRoot, startupNonce }).catch((error) => {
    try {
      const paths = runtimePaths(workspaceRoot);
      prepareRuntime(paths);
      writeStartupError(paths, {
        nonce: startupNonce,
        code: error.code || "startupFailed",
        message: conciseError(error),
      });
      removeOwnedStartupLock(paths, startupNonce, process.pid, "daemon");
    } catch {}
    process.stderr.write(`${JSON.stringify(errorEnvelope(error.code || "startupFailed", conciseError(error)))}\n`);
    process.exitCode = 1;
  });
}
