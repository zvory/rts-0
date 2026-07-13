export const SESSION_EXECUTION_LANES = Object.freeze([
  "serialized", "observation", "cancellation", "lifecycle",
]);

export class SessionCoordinator {
  constructor() {
    this.semanticTails = new Map();
  }

  register(sessionId) {
    if (!this.semanticTails.has(sessionId)) this.semanticTails.set(sessionId, Promise.resolve());
  }

  execute(definition, sessionId, operation) {
    if (definition.lane !== "serialized") return operation();
    if (!this.semanticTails.has(sessionId)) {
      return Promise.reject(Object.assign(new Error("Unknown or closing Lab Interact session."), { code: "unknownSession" }));
    }
    const previous = this.semanticTails.get(sessionId);
    const run = previous.then(operation, operation);
    this.semanticTails.set(sessionId, run.catch(() => {}));
    return run;
  }

  drain(sessionId) {
    return this.semanticTails.get(sessionId) || Promise.resolve();
  }

  release(sessionId) {
    this.semanticTails.delete(sessionId);
  }
}
