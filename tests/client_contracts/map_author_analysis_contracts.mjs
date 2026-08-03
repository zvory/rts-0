import assert from "node:assert/strict";

import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { MAP_EDITOR_SYMMETRY, MapEditorSession } from "../../client/src/map_editor_session.js";

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 24, playerCount: 2 });
  let resolveFetch;
  let abortCount = 0;
  const statuses = [];
  const panel = {
    session,
    fetchImpl: () => new Promise((resolve) => { resolveFetch = resolve; }),
    createAbortController: () => ({ signal: {}, abort() { abortCount += 1; } }),
    setTimeoutImpl: () => 41,
    clearTimeoutImpl(id) { assert.equal(id, 41); },
    analysisTimeoutMs: 5000,
    analysisPending: false,
    analysisKind: null,
    analysisResult: null,
    analysisRequestToken: 0,
    analysisAbortController: null,
    analysisTimeoutId: null,
    analysisMapFingerprint: null,
    destroyed: false,
    observedMapDimensions: null,
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    viewport: { setSymmetry() {}, armTool() {}, tool: null },
    setStatus(message, error = false) { this.status = message; statuses.push({ message, error }); },
    render() {},
  };
  const unsubscribe = session.subscribe((snapshot) => MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot));
  const pending = MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "check");
  assert.equal(panel.analysisPending, true);
  session.mutate("Changed while checking", (draft) => { draft.description = "new revision"; });
  assert.equal(abortCount, 1, "editing the analyzed map aborts its in-flight request");
  assert.equal(panel.analysisPending, false);
  assert.equal(panel.analysisResult, null);
  assert.equal(panel.analysisKind, null);
  assert.equal(panel.status, "", "editing during analysis clears its no-longer-applicable progress banner");
  resolveFetch({
    ok: true,
    async json() { return { valid: true, baseSites: [{}, {}], startLocations: [{}, {}] }; },
  });
  assert.equal(await pending, null);
  assert.equal(panel.analysisResult, null, "a response for an old map fingerprint is ignored");
  assert.equal(statuses.filter((status) => status.message.includes("passed")).length, 0,
    "a stale completion never reports success for the edited map");
  unsubscribe();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 24, playerCount: 2 });
  const panel = {
    session,
    fetchImpl: async () => ({
      ok: true,
      async json() { return { valid: true, baseSites: [{}, {}], startLocations: [{}, {}] }; },
    }),
    createAbortController: () => ({ signal: {}, abort() {} }),
    setTimeoutImpl: () => 3,
    clearTimeoutImpl() {},
    analysisPending: false,
    analysisKind: null,
    analysisResult: null,
    analysisRequestToken: 0,
    analysisAbortController: null,
    analysisTimeoutId: null,
    analysisMapFingerprint: null,
    analysisStatusOwned: false,
    destroyed: false,
    observedMapDimensions: null,
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    viewport: { setSymmetry() {}, armTool() {}, tool: null },
    setStatus(message, error = false) { this.status = message; this.statusError = error; },
    render() {},
  };
  const unsubscribe = session.subscribe((snapshot) => MapEditorPanel.prototype.applySessionSnapshot.call(panel, snapshot));
  await MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "check");
  assert.match(panel.status, /Authoritative check passed/);
  assert.equal(panel.analysisResult.valid, true);
  session.mutate("Edit after completed check", (draft) => { draft.name = "New revision"; });
  assert.equal(panel.analysisResult, null);
  assert.equal(panel.status, "", "editing clears a completed authoritative summary owned by the old map");
  await MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "check");
  MapEditorPanel.prototype.setStatus.call(panel, "Selected water brush.");
  session.mutate("Edit after unrelated status", (draft) => { draft.description = "Changed again"; });
  assert.equal(panel.status, "Selected water brush.", "analysis invalidation does not erase a newer unrelated status");
  unsubscribe();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 24, playerCount: 2 });
  let timeout;
  let aborted = false;
  const statuses = [];
  const panel = {
    session,
    fetchImpl: () => new Promise(() => {}),
    createAbortController: () => ({ signal: {}, abort() { aborted = true; } }),
    setTimeoutImpl(callback) { timeout = callback; return 9; },
    clearTimeoutImpl() {},
    analysisTimeoutMs: 25,
    analysisPending: false,
    analysisKind: null,
    analysisResult: null,
    analysisRequestToken: 0,
    destroyed: false,
    setStatus(message, error = false) { statuses.push({ message, error }); },
    render() {},
  };
  const pending = MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "report");
  timeout();
  assert.equal(await pending, null);
  assert.equal(aborted, true, "the total analysis deadline aborts the underlying fetch");
  assert.match(statuses.at(-1).message, /timed out after 25 ms/);
  assert.equal(statuses.at(-1).error, true);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 24, playerCount: 2 });
  let resolveFetch;
  let aborted = false;
  const statuses = [];
  const panel = {
    session,
    fetchImpl: () => new Promise((resolve) => { resolveFetch = resolve; }),
    createAbortController: () => ({ signal: {}, abort() { aborted = true; } }),
    setTimeoutImpl: () => 17,
    clearTimeoutImpl() {},
    analysisPending: false,
    analysisKind: null,
    analysisResult: null,
    analysisRequestToken: 0,
    destroyed: false,
    setStatus(message, error = false) { statuses.push({ message, error }); },
    render() {},
  };
  const pending = MapEditorPanel.prototype.runAuthoritativeAnalysis.call(panel, "check");
  panel.destroyed = true;
  MapEditorPanel.prototype.invalidateAuthoritativeAnalysis.call(panel);
  resolveFetch({ ok: true, async json() { return { valid: true, baseSites: [], startLocations: [] }; } });
  assert.equal(await pending, null);
  assert.equal(aborted, true, "destroy aborts an in-flight authoritative request");
  assert.equal(statuses.filter((status) => status.message.includes("passed")).length, 0,
    "a completion after destroy does not update panel status");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.mutate("Directional road field", (draft) => { draft.terrain = Array(16).fill("-".repeat(16)); });
  const warnings = MapEditorPanel.prototype.currentSymmetryWarnings.call({
    session,
    symmetry: MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN,
  });
  assert(warnings.some((warning) => warning.includes("terrain") && warning.includes("diagonalMain")),
    "the editor exposes shared warnings for its current map and selected symmetry");
}
