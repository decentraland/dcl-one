import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

// The exported realm uses the standalone app's root-relative content URL. The
// engine consumes baseUrl verbatim (including in native URL parsing), so expose
// a realm for Sites with this deployment's absolute /play/ content address.
// Nix fixes PLAY_WEB_DIR to the packaged client; dev uses the verified import.
export async function loader({ request }: { request: Request }) {
  const directory = process.env.PLAY_WEB_DIR || "../../.cache/play/current";
  const realm = JSON.parse(await readFile(resolve(directory, "bridge-scene/static/BevyExplorerUI/about"), "utf8"));
  const urns: unknown = realm.configurations?.scenesUrn;
  if (!Array.isArray(urns) || urns.length !== 1 || typeof urns[0] !== "string") {
    throw new Response("Invalid bundled bridge realm", { status: 503 });
  }
  const match = /^urn:decentraland:entity:(baf[a-z2-7]+)\?=&baseUrl=\/bridge-scene\/static\/$/.exec(urns[0]);
  if (!match) throw new Response("Invalid bundled bridge content URL", { status: 503 });
  const content = new URL("/play/bridge-scene/static/", request.url);
  // Sites is private behind the gateway, which overwrites this header with its
  // resolved scheme. react-router-serve itself sees the internal HTTP hop.
  const scheme = request.headers.get("x-forwarded-proto");
  if (scheme === "https" || scheme === "http") content.protocol = scheme + ":";
  return Response.json(
    { ...realm, configurations: { ...realm.configurations,
      scenesUrn: [`urn:decentraland:entity:${match[1]}?=&baseUrl=${content.href}`] } },
    { headers: { "Cache-Control": "no-cache" } },
  );
}
