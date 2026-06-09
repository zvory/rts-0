import { Match } from "./match.js";

export class ReplayViewer extends Match {
  constructor(net, payload, toast, devWatch, audio, statusBadge, diagnostics = null) {
    super(net, payload, toast, devWatch, audio, statusBadge, diagnostics, {
      replayViewer: true,
    });
  }
}
