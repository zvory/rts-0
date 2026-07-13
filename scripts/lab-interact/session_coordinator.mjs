export const SESSION_EXECUTION_LANES = Object.freeze([
  "serialized", "observation", "cancellation", "lifecycle",
]);

export class SessionCoordinator {
  constructor() {
    this.semanticTails = new Map();
    this.inFlight = new Map();
  }

  register(sessionId) {
    if (this.semanticTails.has(sessionId)) return;
    this.semanticTails.set(sessionId, Promise.resolve());
    this.inFlight.set(sessionId, new Set());
  }

  execute(definition, sessionId, operation) {
    if (definition.lane === "lifecycle" || sessionId == null) return operation();
    const active = this.inFlight.get(sessionId);
    if (!this.semanticTails.has(sessionId) || !active) {
      return Promise.reject(Object.assign(new Error("Unknown or closing Lab Interact session."), { code: "unknownSession" }));
    }
    if (definition.lane !== "serialized") {
      const run = Promise.resolve().then(operation);
      active.add(run);
      void run.finally(() => active.delete(run)).catch(() => {});
      return run;
    }
    const previous = this.semanticTails.get(sessionId);
    const run = previous.then(operation, operation);
    this.semanticTails.set(sessionId, run.catch(() => {}));
    return run;
  }

  async drain(sessionId) {
    await (this.semanticTails.get(sessionId) || Promise.resolve());
    const active = this.inFlight.get(sessionId);
    if (active?.size) await Promise.allSettled([...active]);
  }

  release(sessionId) {
    this.semanticTails.delete(sessionId);
    this.inFlight.delete(sessionId);
  }
}
