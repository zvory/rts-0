import { readFile } from "node:fs/promises";
import http from "node:http";
import { fileURLToPath } from "node:url";

export const GAME_ORIGINS = Object.freeze({
  mainline: "https://mainline.bewegungskrieg.net",
  beta: "https://beta.bewegungskrieg.net",
});

const indexPath = fileURLToPath(new URL("./index.html", import.meta.url));
const indexHtml = await readFile(indexPath);

export function gameOrigin(channel) {
  return Object.hasOwn(GAME_ORIGINS, channel) ? GAME_ORIGINS[channel] : null;
}

export function gameProbeUrl(channel) {
  const origin = gameOrigin(channel);
  return origin ? `${origin}/version` : null;
}

export function createLauncherServer({ probe = fetch } = {}) {
  return http.createServer(async (request, response) => {
    const requestUrl = new URL(request.url || "/", "http://launcher.invalid");
    if (requestUrl.pathname === "/healthz") {
      response.writeHead(204).end();
      return;
    }

    if (requestUrl.pathname === "/wake") {
      const probeUrl = gameProbeUrl(requestUrl.searchParams.get("channel") || "");
      if (!probeUrl) {
        response.writeHead(400, { "content-type": "application/json" });
        response.end(JSON.stringify({ error: "channel must be beta or mainline" }));
        return;
      }
      try {
        const upstream = await probe(probeUrl, {
          cache: "no-store",
          redirect: "manual",
          signal: AbortSignal.timeout(4000),
        });
        response.writeHead(upstream.ok ? 204 : 503, { "cache-control": "no-store" }).end();
      } catch {
        response.writeHead(503, { "cache-control": "no-store" }).end();
      }
      return;
    }

    response.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
      "content-security-policy": "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'",
      "x-content-type-options": "nosniff",
    });
    response.end(indexHtml);
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const port = Number.parseInt(process.env.PORT || "8080", 10);
  createLauncherServer().listen(port, "0.0.0.0", () => {
    console.log(`launcher listening on 0.0.0.0:${port}`);
  });
}
