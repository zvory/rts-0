const DONE_LINE = String.raw`Done(?:\.[^\n]*)?`;

const DONE_MARKERS = [
  new RegExp(String.raw`^Status:\s*${DONE_LINE}\s*$`, "im"),
  new RegExp(String.raw`^##\s+Status\s*\n+\s*${DONE_LINE}\s*$`, "im"),
  new RegExp(
    String.raw`^##\s+Phase Status\s*\n+(?:[ \t]*\n)*\s*-\s*\[x\]\s*${DONE_LINE}\s*$`,
    "im",
  ),
];

export function phaseMarkedDoneText(text) {
  return DONE_MARKERS.some((marker) => marker.test(text));
}
