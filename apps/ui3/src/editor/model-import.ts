import type { RefObject } from "react";
import type { EditorBus } from "./editor-bus";
import type { ProjectAssets } from "./types";
import { stashProjectModel } from "./project-models";
import { persistCatalogAsset } from "./project-assets";
import { registerProjectContents } from "./project-cache";

export const MODEL_FILE_ACCEPT = ".glb,.gltf,model/gltf-binary,model/gltf+json";
export const MODEL_FILE_RE = /\.(?:glb|gltf)$/i;

export type ImportModelFn = (file: File) => Promise<string>;

export function modelAssetPaths(files: { path: string }[]): string[] {
  return files.map(file => file.path).filter(path => MODEL_FILE_RE.test(path)).sort();
}

// The same path the Local models panel takes for "Import files" and a click
// on the uploaded model, minus the new entity: stash the upload in the project
// cache, copy it into the project's assets, and tell the renderer about it.
export async function importModelFile(busRef: RefObject<EditorBus | null>, file: File, projectAssets?: ProjectAssets): Promise<string> {
  if (!MODEL_FILE_RE.test(file.name)) throw new Error("Only .glb / .gltf files can be added");
  const url = await stashProjectModel(file);
  if (!projectAssets) return url;
  const safe = file.name.replace(/[^\w.-]+/g, "_");
  const src = await persistCatalogAsset(projectAssets, { id: `local:${safe}`, name: safe.replace(MODEL_FILE_RE, ""), glbUrl: url, glbFile: safe });
  await registerProjectContents(busRef, projectAssets);
  return src;
}
