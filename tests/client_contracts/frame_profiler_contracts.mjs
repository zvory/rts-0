import { assert } from "./assertions.mjs";
import {
  RENDER_FRAME_BUDGET_MS,
  RENDER_FRAME_BUDGET_TARGETS,
  buildRenderStressMatrixCells,
  buildRenderStressMatrixSummary,
  buildRenderDiagnosticsReport,
  buildRenderBudgetReport,
  formatRenderBudgetConsole,
  formatRenderStressMatrixMarkdown,
  parseMatrixViewportList,
  parsePositiveNumberList,
} from "../../scripts/client-perf-harness.mjs";
import { FrameProfiler, collectMatchFrameContext } from "../../client/src/frame_profiler.js";
import { validateLiveLabScenarioSample } from "../../scripts/client-perf/workload_setup.mjs";
import {
  buildClientPerfWorkloads,
  defaultClientPerfWorkloads,
} from "../../scripts/client-perf/workloads.mjs";

export function runFrameProfilerContracts() {
  {
    const workloads = buildClientPerfWorkloads({});
    const ids = workloads.map((workload) => workload.id);
    assert(
      ids.filter((id) => id.startsWith("supply-") && !id.includes("hellhole")).length === 0,
      "Hellhole is the sole supply-scale client renderer benchmark",
    );
    const defaultIds = defaultClientPerfWorkloads(workloads).map((workload) => workload.id);
    assert(defaultIds.includes("supply-300-hellhole-stream"), "client-only Hellhole remains in the default renderer workload set");
    const stream = workloads.find((workload) => workload.id === "supply-300-hellhole-stream");
    assert(
      stream?.setup?.snapshotStreamPlayerId === 1
        && stream?.setup?.snapshotStreamSpectator === false
        && JSON.stringify(stream?.setup?.snapshotStreamTeamIds) === JSON.stringify([1, 2, 1, 2])
        && stream?.setup?.snapshotStreamVisibilityTileCount === 126 * 126
        && stream?.setup?.waitForMinEntities === 408,
      "client-only Hellhole measures the full-cadence Player 1 2v2 projection",
    );
    assert(!defaultIds.includes("supply-300-hellhole-integrated"), "live server/client Hellhole is opt-in and cannot contaminate default isolated measurements");
    const integrated = workloads.find((workload) => workload.id === "supply-300-hellhole-integrated");
    assert(integrated?.kind === "labScenario" && integrated?.setup?.waitForMinEntities === 500, "integrated Hellhole retains an explicit canonical Lab view");
    const expectedLab = integrated.setup.liveLabScenario;
    const liveLabSample = {
      scenarioId: expectedLab.scenarioId,
      mapWidth: expectedLab.mapWidth,
      mapHeight: expectedLab.mapHeight,
      projectedEntityCount: expectedLab.projectedEntityCount,
      labMode: true,
      offline: false,
      websocketOpen: true,
    };
    assert(validateLiveLabScenarioSample(liveLabSample, expectedLab).length === 0, "integrated Hellhole accepts exact live Lab identity");
    assert(validateLiveLabScenarioSample({ ...liveLabSample, offline: true }, expectedLab).length > 0, "integrated Hellhole rejects an offline client lane");
    assert(validateLiveLabScenarioSample({ ...liveLabSample, websocketOpen: false }, expectedLab).length > 0, "integrated Hellhole rejects a non-open WebSocket");
  }

  {
    let clock = 0;
    const profiler = new FrameProfiler({
      now: () => clock,
      slowFrameMs: 20,
      slowPhaseMs: 5,
      maxRecentFrames: 2,
    });

    profiler.beginFrame({ at: 0, frameGapMs: 16, scheduledAt: -5 });
    profiler.recordPhase("match.camera", 3);
    profiler.recordPhase("renderer.units", 9);
    profiler.recordPhase("renderer.update", 18);
    profiler.recordPhase("renderer.present", 18);
    profiler.recordPhase("private.localLabel", 11);
    profiler.recordDiagnosticCounter("renderer.pixi.displayObject.created.units", 2);
    profiler.recordDiagnosticCounter("renderer.pixi.displayObject.created.units", 1);
    profiler.recordDiagnosticCounter("private.counter.label", 99);
    profiler.endFrame({ at: 25, context: { entityCount: 7, selectedCount: 2, hidden: false, focused: true } });

    clock = 40;
    profiler.beginFrame({ at: 40, frameGapMs: 40 });
    profiler.recordDiagnosticCounter("hud.dirty.resources.hit", 1);
    profiler.time("match.hud", () => { clock = 47; });
    profiler.endFrame({ at: 48, context: { visibleTileCount: 12 } });

    clock = 70;
    profiler.beginFrame({ at: 70, frameGapMs: 10 });
    profiler.recordPhase("renderer.units", 1);
    profiler.endFrame({ at: 72 });

    const summary = profiler.summary();
    assert(summary.schemaVersion === 1, "FrameProfiler exposes a versioned debug summary");
    assert(summary.frameCount === 3, "FrameProfiler counts completed frames");
    assert(summary.slowFrameCount === 2, "FrameProfiler counts slow frames by gap or work");
    assert(summary.recentFrames.length === 2, "FrameProfiler keeps recent frame history bounded");
    assert(summary.context.entityCount === 7, "FrameProfiler preserves latest entity count context");
    assert(summary.context.visibleTileCount === 12, "FrameProfiler merges later shape context");
    const unitsPhase = summary.phases.find((phase) => phase.label === "renderer.units");
    assert(unitsPhase?.count === 2, "FrameProfiler aggregates repeated renderer phases");
    assert(unitsPhase?.slowCount === 1, "FrameProfiler counts slow phase samples");
    assert(unitsPhase?.maxMs === 9, "FrameProfiler records phase max timing");
    assert(unitsPhase?.p50Ms === 1, "FrameProfiler reports bucketed p50 timing");
    assert(unitsPhase?.p95Ms === 12, "FrameProfiler reports bucketed p95 timing");
    const unattributedPhase = summary.phases.find((phase) => phase.label === "frame.unattributed");
    assert(unattributedPhase?.p95Ms === 24, "FrameProfiler records unattributed frame work");
    const rafDispatchPhase = summary.phases.find((phase) => phase.label === "frame.rafDispatch");
    assert(rafDispatchPhase?.p95Ms === 8, "FrameProfiler records RAF dispatch delay separately");
    assert(summary.worstPhase?.label === "frame.unattributed", "FrameProfiler can report missing frame attribution as the worst phase");
    assert(summary.recentLongFrames.length === 2, "FrameProfiler keeps bounded long-frame context");
    assert(summary.recentLongFrames[0].rafDispatchMs === 5, "FrameProfiler long-frame context includes RAF dispatch delay");
    assert(summary.recentLongFrames[0].unattributedFrameMs === 22, "FrameProfiler long-frame context includes unattributed work");
    assert(
      summary.recentLongFrames[0].rendererNestedPhase?.label === "renderer.present",
      "FrameProfiler long-frame context names the slowest nested renderer phase",
    );
    const createdCounter = summary.renderDiagnostics.counters.find(
      (counter) => counter.label === "renderer.pixi.displayObject.created.units",
    );
    assert(createdCounter?.total === 3, "FrameProfiler aggregates diagnostic counter totals");
    assert(createdCounter?.frames === 1, "FrameProfiler counts frames where a diagnostic counter appeared");
    assert(createdCounter?.maxFrame === 3, "FrameProfiler records diagnostic max per frame");
    assert(profiler.text().includes("renderer.units"), "FrameProfiler text summary is copyable");
    assert(profiler.text().includes("renderer.pixi.displayObject.created.units"), "FrameProfiler text includes diagnostics");
    const report = profiler.reportSummary();
    assert(report.frameCount === 3, "FrameProfiler report summary counts the report window");
    assert(report.slowFrameCount === 2, "FrameProfiler report summary counts slow frames");
    assert(report.frameWorkMaxMs === 25, "FrameProfiler report summary records max frame work");
    assert(report.frameWorkP95Ms === 33, "FrameProfiler report summary records bucketed frame work p95");
    assert(report.frameUnattributedMaxMs === 22, "FrameProfiler report summary records max unattributed frame work");
    assert(report.frameUnattributedP95Ms === 24, "FrameProfiler report summary records bucketed unattributed p95");
    assert(report.frameRafDispatchMaxMs === 5, "FrameProfiler report summary records max RAF dispatch delay");
    assert(report.frameRafDispatchP95Ms === 8, "FrameProfiler report summary records bucketed RAF dispatch p95");
    assert(report.worstFramePhase === "frame.unattributed", "FrameProfiler report summary names worst phase");
    assert(report.worstFramePhaseMs === 22, "FrameProfiler report summary records worst phase max");
    assert(report.rendererMaxMs === 0, "FrameProfiler report summary tolerates missing renderer phase");
    assert(report.rendererUpdateMaxMs === 18 && report.rendererUpdateP95Ms === 24, "renderer update max/p95 are stable report scalars");
    assert(report.rendererPresentMaxMs === 18 && report.rendererPresentP95Ms === 24, "actual present max/p95 are stable report scalars");
    assert(report.frameWorkBudgetMissCount === 1, "FrameProfiler counts complete frame work above the 60 FPS budget");
    assert(report.presentBudgetMissCount === 1, "FrameProfiler counts presents above the 60 FPS budget");
    assert(
      report.clientFramePhases.some((phase) => phase.label === "frame.unattributed") &&
        !report.clientFramePhases.some((phase) => phase.label === "private.localLabel"),
      "FrameProfiler upload phase summary uses stable allowlisted labels",
    );
    assert(report.rendererFramePhases[0].label === "renderer.present", "FrameProfiler upload names top renderer subphase");
    assert(report.topRendererPhase === "renderer.present", "FrameProfiler upload exposes top renderer phase scalar");
    assert(
      report.renderDiagnosticCounters.some((counter) => counter.label === "renderer.pixi.displayObject") &&
        report.renderDiagnosticCounters.every((counter) => counter.label !== "private.counter.label"),
      "FrameProfiler upload diagnostics are grouped through an allowlist",
    );
    assert(
      report.topRenderDiagnosticGroup === "renderer.pixi.displayObject",
      "FrameProfiler upload exposes the top render diagnostic group",
    );
    assert(
      report.renderDiagnostics.counters.some((counter) => counter.label === "hud.dirty.resources.hit"),
      "FrameProfiler report summary includes bounded diagnostics",
    );
    const surface = profiler.debugSurface();
    assert(typeof surface.summary === "function", "FrameProfiler debug surface exposes summary()");
    assert(typeof surface.copy === "function", "FrameProfiler debug surface exposes copy()");
    assert(typeof surface.reportSummary === "function", "FrameProfiler debug surface exposes reportSummary()");
    profiler.resetReportWindow();
    assert(profiler.reportSummary().frameCount === 0, "FrameProfiler can reset only report-window aggregates");
    assert(profiler.reportSummary().frameWorkBudgetMissCount === 0 && profiler.reportSummary().presentBudgetMissCount === 0, "report reset clears 60 FPS budget-miss counts");
    assert(profiler.reportSummary().renderDiagnostics.counters.length === 0, "FrameProfiler report reset clears diagnostics");
    assert(profiler.summary().frameCount === 3, "FrameProfiler report-window reset preserves debug aggregates");
    surface.reset();
    assert(profiler.summary().frameCount === 0, "FrameProfiler debug surface reset clears aggregates");
    assert(profiler.summary().renderDiagnostics.counters.length === 0, "FrameProfiler reset clears diagnostics");
  }

  {
    const report = buildRenderBudgetReport({
      schemaVersion: 1,
      frameCount: 120,
      slowFrameCount: 2,
      worstPhase: { label: "match.minimap", count: 80 },
      context: { entityCount: 42, selectedCount: 4 },
      phases: [
        { label: "frame.work", count: 120, avgMs: 7.5, maxMs: 14.6, p50Ms: 8, p95Ms: 12, slowCount: 0 },
        { label: "frame.unattributed", count: 120, avgMs: 4.9, maxMs: 10, p50Ms: 4, p95Ms: 8, slowCount: 4 },
        { label: "match.minimap", count: 120, avgMs: 2.6, maxMs: 5.9, p50Ms: 2, p95Ms: 4, slowCount: 0 },
        { label: "renderer.units", count: 120, avgMs: 0.9, maxMs: 2.4, p50Ms: 1, p95Ms: 2, slowCount: 0 },
      ],
    });

    assert(report.target.fps === 240, "render budget report exposes the 240 FPS target");
    assert(report.target.frameBudgetMs === RENDER_FRAME_BUDGET_MS, "render budget report exposes the 240 FPS frame budget");
    assert(report.target.frameBudgets.length === RENDER_FRAME_BUDGET_TARGETS.length, "render budget report exposes all FPS frame budgets");
    assert(report.status === "warn", "render budget report warns without failing on over-budget frame work");
    assert(report.frameWork.avgMs === 7.5, "render budget report includes frame.work average");
    assert(report.frameWork.p95Ms === 12, "render budget report includes frame.work p95");
    assert(report.frameAttribution.topLevelAvgMs === 2.6, "render budget report sums top-level named work");
    assert(report.frameAttribution.unattributedP95Ms === 8, "render budget report includes unattributed p95");
    const budget120 = report.frameWork.budgetMargins.find((budget) => budget.fps === 120);
    assert(budget120.p95MarginMs === -3.67 && budget120.p95Clears === false, "render budget report shows p95 margin to 120 FPS");
    assert(report.frameWork.nextMissedBudget.fps === 120, "render budget report names the next missed p95 budget");
    assert(report.worstPhase.label === "match.minimap", "render budget report preserves worst-phase count context");
    assert(
      report.recurringPhaseWarnings.some((phase) => phase.label === "match.minimap" && phase.severity === "high"),
      "render budget report calls out recurring phases above 2 ms",
    );
    assert(
      report.groups.topLevel.some((phase) => phase.label === "match.minimap")
        && report.groups.rendererNested.some((phase) => phase.label === "renderer.units"),
      "render budget report separates top-level match phases from nested renderer phases",
    );
    assert(
      formatRenderBudgetConsole(report).includes("next missed=120 FPS"),
      "render budget console summary shows the next missed budget",
    );
    assert(
      formatRenderBudgetConsole(report).includes("advisory"),
      "render budget console summary labels warnings as advisory",
    );
    assert(
      formatRenderBudgetConsole(report).includes("frame attribution"),
      "render budget console summary includes frame attribution",
    );
  }

  {
    const report = buildRenderBudgetReport({
      schemaVersion: 1,
      frameCount: 120,
      slowFrameCount: 0,
      phases: [
        { label: "frame.work", count: 120, avgMs: 3.8, maxMs: 9, p50Ms: 4, p95Ms: 7.5, slowCount: 0 },
      ],
    });

    assert(report.frameWork.nextMissedBudget.fps === 240, "120 FPS work can still miss the 240 FPS target");
    assert(
      report.warnings.some((warning) => warning.kind === "frame_work_p95_over_budget" && warning.severity === "high"),
      "render budget report treats a missed 240 FPS target as high severity",
    );
  }

  {
    const report = buildRenderBudgetReport({
      schemaVersion: 1,
      frameCount: 120,
      slowFrameCount: 0,
      phases: [
        { label: "frame.work", count: 120, avgMs: 3, maxMs: 5, p50Ms: 2, p95Ms: 4, slowCount: 0 },
      ],
    });

    assert(report.frameWork.nextMissedBudget.fps === 480, "240 FPS work can still miss the next headroom target");
    assert(
      report.warnings.some((warning) =>
        warning.kind === "frame_work_p95_misses_headroom_budget"
          && warning.message.includes("clears 240 FPS locally")
      ),
      "render budget report reserves headroom warnings for budgets above the 240 FPS target",
    );
  }

  {
    const missing = buildRenderDiagnosticsReport(null, null);
    assert(missing.status === "missing", "render diagnostics report tolerates absent counters");

    const report = buildRenderDiagnosticsReport({
      schemaVersion: 1,
      context: { workloadId: "vehicle-wall-stress" },
      renderDiagnostics: {
        schemaVersion: 1,
        counters: [
          { label: "renderer.rig.redraw.completed", total: 20, frames: 5, maxFrame: 6 },
          { label: "minimap.invalidate.fog.fog-revision", total: 4, frames: 4, maxFrame: 1 },
          { label: "hud.dirty.resources.hit", total: 12, frames: 12, maxFrame: 1 },
        ],
      },
      recentLongFrames: [
        {
          at: 12,
          frameWorkMs: 34,
          topPhase: { label: "match.renderer", ms: 18 },
          rendererNestedPhase: { label: "renderer.units", ms: 14 },
        },
      ],
    });
    assert(report.status === "ok", "render diagnostics report summarizes present counters");
    assert(report.groups.rigRedraws.total === 20, "render diagnostics groups rig redraw counters");
    assert(report.groups.minimapInvalidations.total === 4, "render diagnostics groups minimap invalidations");
    assert(report.recentLongFrames[0].rendererNestedPhase.label === "renderer.units", "render diagnostics preserves long-frame context");
  }

  {
    const cpus = parsePositiveNumberList("1,2,4", "--matrix-cpu");
    const dprs = parsePositiveNumberList("1,1.5,2", "--matrix-dpr");
    const viewports = parseMatrixViewportList("small,1440x900,large");
    assert(cpus.length === 3 && cpus[2] === 4, "stress matrix parser accepts CPU throttle lists");
    assert(dprs.includes(1.5), "stress matrix parser accepts fractional DPR values");
    assert(
      viewports.some((viewport) => viewport.label === "small" && viewport.width === 1024)
        && viewports.some((viewport) => viewport.label === "1440x900" && viewport.height === 900),
      "stress matrix parser accepts presets and explicit viewport sizes",
    );

    const workloads = [{ id: "vehicle-wall-stress" }, { id: "selected-unit-hud-stress" }];
    const cells = buildRenderStressMatrixCells({
      workloads,
      cpuThrottles: [1, 4],
      viewports: viewports.slice(0, 2),
      deviceScaleFactors: [1, 2],
      repeatCount: 2,
    });
    assert(cells.length === 32, "stress matrix expands workloads, CPU, viewport, DPR, and repeats");
    assert(
      cells.some((cell) => cell.configLabel.includes("cpu4") && cell.configLabel.includes("dpr2")),
      "stress matrix cells include stable config labels",
    );

    const passingBudget = buildRenderBudgetReport({
      schemaVersion: 1,
      frameCount: 120,
      phases: [
        { label: "frame.work", count: 120, avgMs: 3, maxMs: 4, p95Ms: 3.5 },
        { label: "match.renderer", count: 120, avgMs: 0.8, maxMs: 1.2, p95Ms: 1 },
      ],
    });
    const failingBudget = buildRenderBudgetReport({
      schemaVersion: 1,
      frameCount: 120,
      worstPhase: { label: "match.renderer", count: 80 },
      phases: [
        { label: "frame.work", count: 120, avgMs: 12, maxMs: 22, p95Ms: 18 },
        { label: "frame.unattributed", count: 120, avgMs: 7, maxMs: 15, p95Ms: 14 },
        { label: "match.renderer", count: 120, avgMs: 5, maxMs: 12, p95Ms: 9 },
        { label: "renderer.units", count: 120, avgMs: 3, maxMs: 7, p95Ms: 5 },
      ],
    });
    const matrixSummary = buildRenderStressMatrixSummary([
      {
        status: "passed",
        workloadId: "vehicle-wall-stress",
        artifactDir: "target/client-perf/vehicle-wall-stress/a",
        renderBudget: passingBudget,
        matrixCell: {
          workloadId: "vehicle-wall-stress",
          configLabel: "cpu1-vpdefault-dpr1",
          cpuThrottleRate: 1,
          viewport: { label: "default", width: 1440, height: 900 },
          deviceScaleFactor: 1,
          repeatIndex: 1,
          repeatCount: 1,
        },
      },
      {
        status: "passed",
        workloadId: "selected-unit-hud-stress",
        artifactDir: "target/client-perf/selected-unit-hud-stress/a",
        renderBudget: failingBudget,
        matrixCell: {
          workloadId: "selected-unit-hud-stress",
          configLabel: "cpu4-vplarge-dpr2",
          cpuThrottleRate: 4,
          viewport: { label: "large", width: 1920, height: 1080 },
          deviceScaleFactor: 2,
          repeatIndex: 1,
          repeatCount: 1,
        },
      },
    ], { durationMs: 1000, matrixRepeatCount: 1 });
    assert(matrixSummary.cells.length === 2, "stress matrix summary groups runs into cells");
    assert(
      matrixSummary.firstFailingCell.workloadId === "selected-unit-hud-stress",
      "stress matrix summary ranks the first failing cell",
    );
    assert(
      matrixSummary.firstFailingCell.topMeasuredPhase.label === "frame.unattributed",
      "stress matrix summary reports unattributed work when it is the top measured phase",
    );
    assert(
      formatRenderStressMatrixMarkdown(matrixSummary).includes("selected-unit-hud-stress"),
      "stress matrix markdown includes failing workload rows",
    );
  }

  {
    const priorWindow = globalThis.window;
    const priorDocument = globalThis.document;
    globalThis.window = { devicePixelRatio: 2, __rtsPerfWorkloadId: "selected-unit-hud-stress" };
    globalThis.document = { hidden: true, hasFocus: () => false };
    try {
      const context = collectMatchFrameContext({
        lastSnapshotTick: 123,
        state: {
          _curById: new Map([[1, {}], [2, {}], [3, {}]]),
          selection: new Set([1, 2]),
          rememberedBuildings: [{ id: 9 }],
          visibleTiles: Uint8Array.from([1, 0, 1, 1]),
        },
        camera: {
          projectionSnapshot: () => ({
            camera: { version: 1, focus: { x: 400, y: 300 }, framingScale: 1.5, boundsPolicy: "mapOverscroll" },
            viewport: { widthCssPx: 800, heightCssPx: 600 },
          }),
        },
        renderer: { app: { view: { width: 1600, height: 1200 }, renderer: {} } },
        prediction: { debugSummary: () => ({ mode: "predicting" }) },
      });
      assert(context.matchMode === "live", "match frame context includes bounded mode");
      assert(context.workloadId === "selected-unit-hud-stress", "match frame context includes local workload id");
      assert(context.matchTick === 123, "match frame context includes latest match tick");
      assert(context.entityCount === 3, "match frame context includes current entity count");
      assert(context.selectedCount === 2, "match frame context includes selected count");
      assert(context.rememberedBuildingCount === 1, "match frame context includes remembered building count");
      assert(context.visibleTileCount === 3, "match frame context counts visible tiles");
      assert(context.canvasWidth === 1600, "match frame context includes canvas backing width");
      assert(context.devicePixelRatio === 2, "match frame context includes device pixel ratio");
      assert(context.predictionMode === "predicting", "match frame context includes prediction mode");
      assert(context.hidden === true && context.focused === false, "match frame context includes document state");
    } finally {
      if (priorWindow === undefined) delete globalThis.window;
      else globalThis.window = priorWindow;
      if (priorDocument === undefined) delete globalThis.document;
      else globalThis.document = priorDocument;
    }
  }
}
