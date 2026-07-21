export async function prepareRenderer(canvasParent, backendBundle) {
  const source = { match: null };
  const renderer = await backendBundle.createRenderer(canvasParent, {
    state: () => source.match?.state,
    profiler: () => source.match?.frameProfiler,
    visualProfile: () => source.match?.visualProfile,
    staticMap: () => source.match?.presentationAssembler?.staticMap,
  });
  return {
    backendBundle,
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
