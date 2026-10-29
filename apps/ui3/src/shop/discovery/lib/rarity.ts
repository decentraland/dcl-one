// from: shop@94584a5c:app/src/lib/rarity.ts
// Rarity colours, labels and scarcity. The palette is ui3's --rar-* tokens (atoms/primitives.css),
// not upstream's Figma theme, so the Shop's chips match the rest of ui3; the maximum supply per
// rarity is @dcl/schemas' Rarity.getMaxSupply.

import { t } from "../intl";

export const RARITIES = ["common", "uncommon", "epic", "rare", "legendary", "exotic", "mythic", "unique"];

const COLORS: Record<string, string> = {
  common: "#73d3d3",
  uncommon: "#ff8362",
  rare: "#34ce76",
  epic: "#438fff",
  legendary: "#a755f4",
  exotic: "#9bd141",
  mythic: "#ff4bed",
  unique: "#fea217",
};

export const MAX_SUPPLY: Record<string, number> = {
  common: 100_000,
  uncommon: 10_000,
  rare: 5_000,
  epic: 1_000,
  legendary: 100,
  exotic: 50,
  mythic: 10,
  unique: 1,
};

const FALLBACK_COLOR = "#E6E6E6";

function parseHex(color: string): [number, number, number] | null {
  const h = typeof color === "string" ? color.replace("#", "") : "";
  if (h.length < 6) return null;
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

function luminance([r, g, b]: [number, number, number]): number {
  return 0.299 * r + 0.587 * g + 0.114 * b;
}

export function rarityColor(rarity?: string | null): string {
  if (!rarity) return FALLBACK_COLOR;
  return COLORS[rarity.toLowerCase()] ?? FALLBACK_COLOR;
}

export function rarityTint(rarity?: string | null, alpha = 0.3): string {
  const rgb = parseHex(rarityColor(rarity));
  if (!rgb) return `rgba(160, 155, 168, ${alpha})`;
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
}

const GLOW_COLORS: Record<string, { color: string; saturation?: number }> = {
  exotic: { color: "#44c75b", saturation: 0.81 },
  rare: { color: "#3fd39a" },
};

function glowEntry(rarity?: string | null) {
  return GLOW_COLORS[(rarity ?? "").toLowerCase()];
}

function glowColor(rarity?: string | null): string {
  return glowEntry(rarity)?.color || rarityColor(rarity);
}

/** The glow's outer halo as a bare "r g b" triple. */
export function rarityGlowRgb(rarity?: string | null): string {
  const rgb = parseHex(glowColor(rarity));
  return rgb ? rgb.join(" ") : "160 155 168";
}

/** The glow's centre: the rarity's hue at a fixed saturation and lightness, as "r g b". */
export function rarityGlowCoreRgb(rarity?: string | null, lightness = 0.66, saturation?: number): string {
  const rgb = parseHex(glowColor(rarity));
  if (!rgb) return "160 155 168";
  const sat = saturation ?? glowEntry(rarity)?.saturation ?? 0.95;
  const [r, g, b] = rgb.map((c) => c / 255) as [number, number, number];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  if (!delta) return rgb.join(" ");
  let hue: number;
  if (max === r) hue = (g - b) / delta;
  else if (max === g) hue = (b - r) / delta + 2;
  else hue = (r - g) / delta + 4;
  hue = (((hue * 60) % 360) + 360) % 360;
  const chroma = (1 - Math.abs(2 * lightness - 1)) * sat;
  const second = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
  const lift = lightness - chroma / 2;
  const sectors: [number, number, number][] = [
    [chroma, second, 0],
    [second, chroma, 0],
    [0, chroma, second],
    [0, second, chroma],
    [second, 0, chroma],
    [chroma, 0, second],
  ];
  const sector = sectors[Math.floor(hue / 60) % 6]!;
  return sector.map((channel) => Math.round((channel + lift) * 255)).join(" ");
}

/** Text colour for a tinted rarity chip: the hue, darkened until it reads on the pale fill. */
export function rarityInk(rarity?: string | null, target = 120): string {
  const color = rarityColor(rarity);
  const rgb = parseHex(color);
  if (!rgb) return "#161518";
  const lum = luminance(rgb);
  if (lum <= target) return color;
  const k = target / lum;
  const hex = (n: number) => Math.round(n * k).toString(16).padStart(2, "0");
  return `#${hex(rgb[0])}${hex(rgb[1])}${hex(rgb[2])}`;
}

function capitalizeFirst(s: string): string {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}

export function rarityLabel(rarity?: string | null): string {
  const key = (rarity ?? "").toLowerCase();
  return RARITIES.includes(key) ? t(`rarity.${key}`) : capitalizeFirst(rarity ?? "");
}

export function rarityDescription(rarity?: string | null): string {
  const name = rarityLabel(rarity ?? "common");
  const max = MAX_SUPPLY[(rarity ?? "common").toLowerCase()];
  return max ? t("rarity.supply", { rarity: name, max: max.toLocaleString("en-US") }) : t("rarity.plain", { rarity: name });
}

/** The media area's background: the rarity's hue gathering toward the edges. */
export function rarityMedia(rarity?: string | null): string {
  const rgb = parseHex(rarityColor(rarity));
  if (!rgb) return "none";
  const [r, g, b] = rgb;
  return `radial-gradient(circle at 50% 38%, rgba(${r}, ${g}, ${b}, 0.04) 0%, rgba(${r}, ${g}, ${b}, 0.3) 50%, rgba(${r}, ${g}, ${b}, 0.62) 100%)`;
}
