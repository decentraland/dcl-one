import { z } from "zod";

import { worldsContentServerUrl } from "./bases";

export function wcsBase(override?: string): string {
  return worldsContentServerUrl(override);
}

export const WCS_LIVE_DATA_PATH = "/live-data";

export const LiveDataSchema = z.object({
  data: z.object({
    totalUsers: z.number(),
    perWorld: z.array(
      z.object({
        worldName: z.string(),
        users: z.number(),
      }),
    ),
  }),
  lastUpdated: z.iso.datetime({ offset: true }).nullish().transform((v) => v ?? null),
});
