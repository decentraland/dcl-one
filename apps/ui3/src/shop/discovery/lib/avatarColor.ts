// from: shop@2d3bf62:app/src/lib/avatarColor.ts
const FNV_OFFSET_BASIS = 0x811c9dc5;
const FNV_PRIME = 0x01000193;
const UINT32_MAX = 0xffffffff;
const DEFAULT_COLOR = "#ffffff";
const ALPHANUMERIC = /[\p{L}\p{N}]/u;

export function fnv1a32(input: string): number {
  let hash = FNV_OFFSET_BASIS >>> 0;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, FNV_PRIME) >>> 0;
  }
  return hash >>> 0;
}

function hsvToRgb(hue: number, s: number, v: number): [number, number, number] {
  const h = hue >= 1 ? hue - Math.floor(hue) : hue;
  const sextant = Math.floor(h * 6);
  const f = h * 6 - sextant;
  const p = v * (1 - s);
  const q = v * (1 - s * f);
  const t = v * (1 - s * (1 - f));
  switch (sextant % 6) {
    case 0:
      return [v, t, p];
    case 1:
      return [q, v, p];
    case 2:
      return [p, v, t];
    case 3:
      return [p, q, v];
    case 4:
      return [t, p, v];
    default:
      return [v, p, q];
  }
}

const hex = (c: number) =>
  Math.max(0, Math.min(255, Math.round(c * 255)))
    .toString(16)
    .padStart(2, "0");

export function getDisplayName(name: string | null | undefined, hasClaimedName: boolean | null | undefined, ethAddress: string | null | undefined): string {
  let validated = "";
  for (const ch of (name ?? "").split("")) if (ALPHANUMERIC.test(ch)) validated += ch;
  if (!validated) return "";
  if (hasClaimedName || !ethAddress || ethAddress.length <= 4) return validated;
  return `${validated}#${ethAddress.slice(-4)}`;
}

export function getAvatarBackgroundColor(displayName: string | null | undefined): string {
  if (!displayName) return DEFAULT_COLOR;
  const [r, g, b] = hsvToRgb(fnv1a32(displayName) / UINT32_MAX, 0.75, 1);
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

// from: shop@3a322cf9:app/src/components/CreatorBadge/CreatorBadge.tsx (colorForAddress, initialFor): the
// fallback disc a creator without a picture gets, one hue per address at mid lightness so the white
// initial stays legible.
export function addressAvatarColor(address: string): string {
  let hash = 0;
  for (let i = 0; i < address.length; i++) hash = (hash * 31 + address.charCodeAt(i)) >>> 0;
  return `hsl(${hash % 360}, 52%, 45%)`;
}

export function avatarInitial(name: string | null | undefined, address: string): string {
  return (name?.trim()?.[0] || address.replace(/^0x/i, "")[0] || "?").toUpperCase();
}
