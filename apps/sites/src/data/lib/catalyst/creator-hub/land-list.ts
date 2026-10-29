import { z } from "zod";

import { getJSON, type GetOptions } from "../client";

const PAGE_SIZE = 1000;
const MAX_PAGES = 100;

const LandSchema = z.object({
  tokenId: z.string(),
  x: z.string(),
  y: z.string(),
  viaEstate: z.boolean(),
});
const DelegatedSchema = z.object({
  id: z.string(),
  x: z.string(),
  y: z.string(),
  owner: z.string(),
});
const PageSchema = z.object({
  elements: z.array(z.unknown()),
  totalAmount: z.number().int().nonnegative(),
});

export type OwnedLand = z.infer<typeof LandSchema>;
export type DelegatedParcel = z.infer<typeof DelegatedSchema>;

async function allPages<T>(
  path: string,
  schema: z.ZodType<T>,
  opts: GetOptions,
): Promise<T[]> {
  const rows: T[] = [];
  for (let pageNum = 1; pageNum <= MAX_PAGES; pageNum++) {
    const raw = await getJSON<unknown>(path, {
      ...opts,
      query: { pageNum, pageSize: PAGE_SIZE },
    });
    const page = PageSchema.parse(raw);
    rows.push(...page.elements.map((entry) => schema.parse(entry)));
    if (rows.length >= page.totalAmount || page.elements.length === 0) return rows;
  }
  throw new Error("LAND list exceeds the supported page limit");
}

export async function loadLandList(
  address: string,
  opts: GetOptions = {},
): Promise<{ owned: OwnedLand[]; delegated: DelegatedParcel[]; delegatedError: string | null }> {
  const addr = address.trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(addr)) throw new Error("Connect a valid wallet");
  const root = `/lambdas/users/${encodeURIComponent(addr)}`;
  const [owned, delegated] = await Promise.allSettled([
    allPages(`${root}/genesis-parcels`, LandSchema, opts),
    allPages(`${root}/lands-permissions`, DelegatedSchema, opts),
  ]);
  if (owned.status === "rejected") throw owned.reason;
  return {
    owned: owned.value,
    delegated: delegated.status === "fulfilled" ? delegated.value : [],
    delegatedError: delegated.status === "rejected"
      ? delegated.reason instanceof Error ? delegated.reason.message : "Publishing grants unavailable"
      : null,
  };
}
