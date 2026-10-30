import { currentProjectScope, restoreProjectCache, type ProjectScope } from "./project-cache-name";
import { runEditorEffect } from "./serial-effect";

export interface CachedProjectModel { path: string; folder: string; url: string }
interface ModelReference { path: string; token: string }
const INVENTORY = "/uploaded-models";
const MODEL = /\.(?:glb|gltf)$/i;
const TOKEN = /^local-[\w.-]+$/;

async function references(cache: Cache, scope: ProjectScope): Promise<ModelReference[]> {
  const response = await cache.match(scope.contentBase + INVENTORY);
  const value: unknown = response ? await response.json().catch(() => null) : null;
  const models = value && typeof value === "object" && "models" in value && Array.isArray(value.models) ? value.models : [];
  const found = new Map<string, ModelReference>();
  for (const model of models) {
    if (model && typeof model.path === "string" && MODEL.test(model.path) && typeof model.token === "string" && TOKEN.test(model.token)) {
      found.set(model.token, { path: model.path, token: model.token });
    }
  }
  // Older scoped drafts stored upload bytes before the inventory existed.
  // Recover names from their documented local-<timestamp>-<filename> keys,
  // looking only inside this project's cache, never another project's cache.
  const prefix = `${scope.contentBase}/contents/`;
  for (const request of await cache.keys()) {
    if (!request.url.startsWith(prefix)) continue;
    const token = request.url.slice(prefix.length);
    if (found.has(token) || !TOKEN.test(token)) continue;
    const path = /^local-(?:[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}|[\da-z]+)-(.+\.(?:glb|gltf))$/i.exec(token)?.[1];
    if (path) found.set(token, { path, token });
  }
  return [...found.values()];
}

export async function loadProjectModels(scope = currentProjectScope()): Promise<CachedProjectModel[]> {
  if (typeof caches === "undefined") return [];
  // The Local panel can mount before the viewport has finished preparation.
  // Join the exact project lineage here instead of racing that background copy.
  await restoreProjectCache(scope);
  const cache = await caches.open(scope.cacheName);
  const models: CachedProjectModel[] = [];
  for (const model of await references(cache, scope)) {
    const url = `${scope.contentBase}/contents/${model.token}`;
    if (await cache.match(url)) models.push({ path: model.path, folder: "uploaded to this project", url });
  }
  return models;
}

export async function stashProjectModel(file: File, scope = currentProjectScope()): Promise<string> {
  if (!MODEL.test(file.name)) throw new Error("Only .glb / .gltf files can be added");
  if (typeof caches === "undefined") throw new Error("Browser project storage is unavailable");
  const safe = file.name.replace(/[^\w.-]+/g, "_");
  const token = `local-${crypto.randomUUID()}-${safe}`;
  const url = `${scope.contentBase}/contents/${token}`;
  const bytes = await file.arrayBuffer();
  await restoreProjectCache(scope);
  return runEditorEffect(`project-models:${scope.runtimeId}`, () => true, async () => {
    const cache = await caches.open(scope.cacheName);
    const models = await references(cache, scope);
    await cache.put(url, new Response(bytes, { headers: {
      "content-type": /\.gltf$/i.test(file.name) ? "model/gltf+json" : "model/gltf-binary",
      "access-control-allow-origin": "*",
    } }));
    await cache.put(scope.contentBase + INVENTORY, Response.json({ version: 1, models: [{ path: file.name, token }, ...models] }));
    return url;
  });
}
