import { z } from "zod";

const LinkSchema = z.object({
  label: z.string(),
  href: z.string(),
});

const CtaTargetSchema = z.object({
  label: z.string(),
  href: z.string(),
});

const HeroSchema = z.object({
  kicker: z.string(),
  title: z.string(),
  mobileTitle: z.string().optional(),
  subtitle: z.string(),
  downloads: z.string().nullish().transform((v) => v ?? null),
  downloadsLabel: z.string(),
  cta: z.object({
    desktop: CtaTargetSchema,
    epic: CtaTargetSchema,
  }),
  platforms: z.array(
    z.object({ id: z.string(), label: z.string(), href: z.string() }),
  ),
});

const EventRailItemSchema = z.object({
  id: z.string(),
  category: z.string(),
  title: z.string(),
  when: z.string(),
  live: z.boolean(),
  image: z.string().nullish().transform((v) => v ?? null),
});

const HotspotRailItemSchema = z.object({
  id: z.string(),
  title: z.string(),
  online: z.number().nullish().transform((v) => v ?? null),
  image: z.string().nullish().transform((v) => v ?? null),
  href: z.string(),
});

const RitualRailItemSchema = z.object({
  id: z.string(),
  title: z.string(),
  day: z.string().nullish().transform((v) => v ?? null),
  image: z.string().nullish().transform((v) => v ?? null),
  href: z.string(),
});

const RailSchema = z.object({
  id: z.string(),
  kind: z.enum(["events", "hotspots", "rituals"]),
  title: z.string(),
  viewAll: LinkSchema.nullish().transform((v) => v ?? null),
  items: z.array(z.unknown()),
});

const HomeContentSchema = z.object({
  hero: HeroSchema,
  rails: z.array(RailSchema),
  comeHangOut: z.object({
    title: z.string(),
    downloads: z.string().nullish().transform((v) => v ?? null),
    downloadsLabel: z.string(),
  }),
});

export type HomeContent = z.infer<typeof HomeContentSchema>;
type Rail = z.infer<typeof RailSchema>;
type EventRailItem = z.infer<typeof EventRailItemSchema>;
export type HotspotRailItem = z.infer<typeof HotspotRailItemSchema>;
export type RitualRailItem = z.infer<typeof RitualRailItemSchema>;

export function parseHomeContent(raw: unknown): HomeContent {
  return HomeContentSchema.parse(raw);
}

function eventItems(rail: Rail): EventRailItem[] {
  return rail.items
    .map((it) => EventRailItemSchema.safeParse(it))
    .filter((r): r is { success: true; data: EventRailItem } => r.success)
    .map((r) => r.data);
}

function hotspotItems(rail: Rail): HotspotRailItem[] {
  return rail.items
    .map((it) => HotspotRailItemSchema.safeParse(it))
    .filter((r): r is { success: true; data: HotspotRailItem } => r.success)
    .map((r) => r.data);
}

function ritualItems(rail: Rail): RitualRailItem[] {
  return rail.items
    .map((it) => RitualRailItemSchema.safeParse(it))
    .filter((r): r is { success: true; data: RitualRailItem } => r.success)
    .map((r) => r.data);
}

export function heroEventCards(content: HomeContent) {
  const rail = content.rails.find((r) => r.kind === "events");
  if (!rail) return undefined;
  const items = eventItems(rail).slice(0, 3);
  if (!items.length) return undefined;
  return items.map((ev) => ({
    id: ev.id,
    cat: ev.category,
    title: ev.title,
    when: ev.live ? "Live now" : ev.when,
    live: ev.live,
    image: ev.image,
  }));
}

export function hotspotCards(content: HomeContent): HotspotRailItem[] | undefined {
  const rail = content.rails.find((r) => r.kind === "hotspots");
  if (!rail) return undefined;
  const items = hotspotItems(rail);
  return items.length ? items : undefined;
}

export function ritualCards(content: HomeContent): RitualRailItem[] | undefined {
  const rail = content.rails.find((r) => r.kind === "rituals");
  if (!rail) return undefined;
  const items = ritualItems(rail);
  return items.length ? items : undefined;
}
