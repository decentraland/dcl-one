import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { signedGetJSON } from "../client";

const ErrorSchema = z.object({
  received_at: z.string(),
  scene_hash: z.string(),
  error_kind: z.string(),
  error_message: z.string(),
});

export type SceneError = z.infer<typeof ErrorSchema>;

export async function fetchSceneErrors(
  identity: AuthIdentity,
  sceneType: "genesis" | "world",
  sceneId: string,
  signal?: AbortSignal,
): Promise<SceneError[]> {
  const raw = await signedGetJSON<unknown>("/api/creator-hub/scene-errors", {
    identity,
    signal,
    query: { type: sceneType, scene: sceneId },
  });
  return z.object({ errors: z.array(ErrorSchema) }).parse(raw).errors;
}
