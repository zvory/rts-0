const SVG_IMAGE_HREF_RE = /(<image\b[^>]*\bhref=")([^"]+)(")/g;

export function createInlineSvgImage(markup, canvas, onReady, { signal = null } = {}) {
  if (typeof markup !== "string" || !markup) return null;
  const doc = canvas?.ownerDocument || globalThis.document || null;
  const image = doc?.createElement
    ? doc.createElement("img")
    : typeof globalThis.Image === "function"
      ? new globalThis.Image()
      : null;
  if (!image) return null;
  image.onload = () => {
    if (!signal?.aborted) onReady?.();
  };
  image.onerror = () => {};
  inlineSvgImageSources(markup, fetchImageDataUrl, { signal })
    .then((selfContainedMarkup) => {
      if (!signal?.aborted) {
        image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(selfContainedMarkup)}`;
      }
    })
    .catch(() => {
      // Leave the image unready so the minimap retains its procedural fallback.
    });
  return image;
}

export async function inlineSvgImageSources(markup, loadDataUrl = fetchImageDataUrl, { signal = null } = {}) {
  if (typeof markup !== "string" || !markup) return "";
  const hrefs = [...new Set([...markup.matchAll(SVG_IMAGE_HREF_RE)].map((match) => match[2]))];
  if (hrefs.length === 0) return markup;
  const dataUrls = new Map(await Promise.all(hrefs.map(async (href) => (
    [href, await loadDataUrl(href, { signal })]
  ))));
  return markup.replace(SVG_IMAGE_HREF_RE, (match, prefix, href, suffix) => {
    const dataUrl = dataUrls.get(href);
    if (typeof dataUrl !== "string" || !dataUrl.startsWith("data:image/")) {
      throw new TypeError(`SVG image source ${href} did not resolve to an image data URL.`);
    }
    return `${prefix}${dataUrl}${suffix}`;
  });
}

async function fetchImageDataUrl(href, { signal = null } = {}) {
  const response = await fetch(href, { credentials: "same-origin", signal });
  if (!response.ok) throw new Error(`Could not load SVG image source ${href}: HTTP ${response.status}`);
  const mimeType = response.headers.get("content-type")?.split(";", 1)[0] || "image/png";
  if (!mimeType.startsWith("image/")) {
    throw new TypeError(`SVG image source ${href} returned ${mimeType}.`);
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return `data:${mimeType};base64,${btoa(binary)}`;
}

// Share source bytes across team-colored portraits, but keep their tinted images independent.
export function createMinimapUnitIconLoader(markupFor, canvas) {
  const sources = new Map();
  return async (kind, color, { signal }) => {
    let markup = await inlineSvgImageSources(markupFor(kind, { teamColor: color }), href => {
      if (!sources.has(href)) sources.set(href, fetchImageDataUrl(href, { signal }));
      return sources.get(href);
    });
    if (!/\bxmlns=/.test(markup)) markup = markup.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"');
    if (signal.aborted) throw new Error("Minimap image load aborted");
    return new Promise((resolve, reject) => {
      const image = canvas.ownerDocument.createElement("img");
      const finish = (error) => {
        image.onload = null; image.onerror = null;
        signal.removeEventListener("abort", abort);
        if (error) { image.src = ""; reject(error); } else resolve(image);
      };
      const abort = () => finish(new Error("Minimap image load aborted"));
      signal.addEventListener("abort", abort, { once: true });
      image.onload = () => finish();
      image.onerror = () => finish(new Error(`Could not decode minimap portrait: ${kind}`));
      image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(markup)}`;
    });
  };
}
