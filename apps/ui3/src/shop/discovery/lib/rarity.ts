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
  epic: "#289cff",
  legendary: "#a24bf3",
  exotic: "#9cd71e",
  mythic: "#ff4bed",
  unique: "#fea217",
};

// The product card's per-rarity tile: `glow` is a radial gradient whose ellipse is `size` of the tile on both axes.
// from: shop@3a322cf9:app/src/styles/theme.ts (rarityCard, rarityCardNone). Upstream keeps `label` apart from the
// rarity colour on purpose: epic's chip text is #438fff and legendary's #b05dff while their flaps (rarityColor) are
// #289cff and #a24bf3.
const CARD: Record<string, { stroke: string; label: string; size: string; glow: string }> = {
  common: { stroke: "#a0f4ff", label: "#73d3d3", size: "74.3%", glow: "rgb(223 244 245) 36.5%, rgb(169 228 228) 68.3%, rgb(115 211 211) 100%" },
  uncommon: { stroke: "#ffb4aa", label: "#ff8362", size: "65.9%", glow: "rgb(255 202 187) 45.7%, rgb(255 167 143) 72.8%, rgb(255 131 98) 100%" },
  rare: { stroke: "#6fefaa", label: "#34ce76", size: "66.4%", glow: "rgb(193 238 207) 60.6%, rgb(158 230 185) 70.4%, rgb(123 222 163) 80.3%, rgb(87 214 140) 90.1%, rgb(52 206 118) 100%" },
  epic: { stroke: "#5dcfff", label: "#438fff", size: "135.5%", glow: "rgb(187 222 251) 2.9%, rgb(150 206 252) 26.9%, rgb(114 189 253) 51%, rgb(77 173 254) 75%, rgb(40 156 255) 99%" },
  legendary: { stroke: "#dba0ff", label: "#b05dff", size: "65.4%", glow: "rgb(223 197 250) 37.5%, rgb(193 136 247) 68.8%, rgb(177 106 245) 84.4%, rgb(162 75 243) 100%" },
  exotic: { stroke: "#caffa0", label: "#9cd71e", size: "76.2%", glow: "rgb(232 255 197) 32.2%, rgb(211 254 138) 66.1%, rgb(200 254 108) 83.1%, rgb(189 253 78) 100%" },
  mythic: { stroke: "#ff95e8", label: "#ff4bed", size: "67.9%", glow: "rgb(255 188 246) 30.8%, rgb(255 132 242) 65.4%, rgb(255 103 239) 82.7%, rgb(255 75 237) 100%" },
  unique: { stroke: "#fdffa0", label: "#fea217", size: "70.3%", glow: "rgb(255 236 181) 40.9%, rgb(255 218 142) 55.6%, rgb(255 199 102) 70.4%, rgb(254 181 63) 85.2%, rgb(254 171 43) 92.6%, rgb(254 162 23) 100%" },
};
const CARD_NONE = { flap: "#a09ba8", stroke: "#a09ba8", label: "#cfcdd4", labelBg: "rgba(236, 235, 237, 0.2)", background: "#ecebed" };

export type RarityCardPalette = { flap: string; stroke: string; label: string; labelBg: string; background: string };

export function rarityCardPalette(rarity?: string | null): RarityCardPalette {
  const entry = CARD[(rarity ?? "").toLowerCase()];
  if (!entry) return CARD_NONE;
  const label = parseHex(entry.label);
  return {
    flap: rarityColor(rarity),
    stroke: entry.stroke,
    label: entry.label,
    labelBg: label ? `rgba(${label[0]}, ${label[1]}, ${label[2]}, 0.2)` : CARD_NONE.labelBg,
    background: `radial-gradient(ellipse ${entry.size} ${entry.size} at 50% 50%, ${entry.glow})`,
  };
}

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
