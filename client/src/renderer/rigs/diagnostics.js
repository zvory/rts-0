export function flushRigDiagnosticCounts(options, labels, counts) {
  if (typeof options.diagnosticRecorder?._recordKnownRenderDiagnostics === "function") {
    options.diagnosticRecorder._recordKnownRenderDiagnostics(labels, counts);
    return;
  }
  if (typeof options.diagnosticBatch === "function") {
    options.diagnosticBatch(labels, counts);
    return;
  }
  const diagnostic = typeof options.diagnostics === "function"
    ? options.diagnostics
    : options.diagnosticRecorder?._recordRenderDiagnostic?.bind?.(options.diagnosticRecorder);
  if (!diagnostic) return;
  for (let i = 0; i < labels.length; i += 1) {
    for (let remaining = counts[i]; remaining > 0; remaining -= 1) {
      diagnostic(labels[i], 1);
    }
  }
}
