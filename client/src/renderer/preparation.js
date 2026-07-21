import { createPixiBackendBundle } from "./backend_bundle.js";

export async function prepareRenderer(canvasParent, backendBundle) {
  const resolvedBackendBundle = backendBundle || createPixiBackendBundle();
  const source = { match: null };
  const renderer = await resolvedBackendBundle.createRenderer(canvasParent, {
    state: () => source.match?.state,
    profiler: () => source.match?.frameProfiler,
    visualProfile: () => source.match?.visualProfile,
    staticMap: () => source.match?.presentationAssembler?.staticMap,
  });
  return {
    backendBundle: resolvedBackendBundle,
    renderer,
    attach(match) {
      source.match = match;
    },
    destroy() {
      source.match = null;
      renderer?.destroy?.();
    },
  };
}
