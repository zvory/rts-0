type JsonObject = Record<string, unknown>;

export class InteractDriverError extends Error {
  details: JsonObject;
  code: string;

  constructor(code: string, message: string, details: JsonObject = {}) {
    super(message);
    this.name = "InteractDriverError";
    this.code = code;
    this.details = details;
  }
}
