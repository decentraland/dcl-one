import { z } from "zod";

export const MappingSchema = z.object({
  from: z.string(),
  legacyPath: z.string(),
  to: z.string(),
  carry: z.array(z.string()),
  param: z
    .object({ name: z.string(), queryKey: z.string().nullable() })
    .nullable(),
});
export type Mapping = z.infer<typeof MappingSchema>;

export const RollupRowSchema = z.object({
  from: z.string(),
  redirects: z.number(),
});

export const RedirectFixtureSchema = z.object({
  _source: z.string(),
  event: z.string(),
  story: z.string(),
  redirectStatus: z.number(),
  mappings: z.array(MappingSchema),
  windowDays: z.number(),
  rollup: z.array(RollupRowSchema),
});
export type RedirectFixture = z.infer<typeof RedirectFixtureSchema>;

export const REDIRECT_DASHBOARD: RedirectFixture = {
  _source: "static-contract",
  event: "creator_builder_redirect",
  story: "creator-integration-redirect-item-publish-curate",
  redirectStatus: 308,
  windowDays: 7,
  mappings: [
    {
      from: "item-editor",
      legacyPath: "/builder/item-editor",
      to: "/create/collections/editor",
      carry: ["collection", "item", "reviewing"],
      param: null,
    },
    {
      from: "publish-collection",
      legacyPath: "/builder/publish-collection",
      to: "/create/collections",
      carry: [],
      param: null,
    },
    {
      from: "curation",
      legacyPath: "/builder/curation",
      to: "/create/curation",
      carry: ["status", "type", "assignee"],
      param: null,
    },
    {
      from: "collection-detail",
      legacyPath: "/builder/collection/:id",
      to: "/create/collections/:id",
      carry: [],
      param: { name: "id", queryKey: null },
    },
    {
      from: "item-detail",
      legacyPath: "/builder/item/:id",
      to: "/create/collections/editor",
      carry: [],
      param: { name: "id", queryKey: "item" },
    },
  ],
  rollup: [],
};

export type DashboardRow = Mapping & {
  redirects: number;
  share: number;
};

export function toDashboardRows(fx: RedirectFixture): DashboardRow[] {
  const counts = new Map(fx.rollup.map((r) => [r.from, r.redirects]));
  const total = fx.rollup.reduce((s, r) => s + r.redirects, 0);
  return fx.mappings
    .map((m) => {
      const redirects = counts.get(m.from) ?? 0;
      return {
        ...m,
        redirects,
        share: total > 0 ? redirects / total : 0,
      };
    })
    .sort((a, b) => b.redirects - a.redirects);
}

export function totalRedirects(fx: RedirectFixture): number {
  return fx.rollup.reduce((s, r) => s + r.redirects, 0);
}

export function pct(ratio: number): string {
  return `${Math.round(ratio * 100)}%`;
}

export function fmtCount(n: number): string {
  return new Intl.NumberFormat("en-US").format(n);
}
