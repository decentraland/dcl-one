// from: sites@17b3535a:src/utils/avatarColor.ts (unity-explorer's NameColorHelper: FNV-1a over the validated
// name, HSV(hue, 0.75, 1)), so a creator's disc has the colour decentraland.org gives it.
const ALPHANUMERIC = /[\p{L}\p{N}]/u;

function fnv1a32(input: string): number {
  let hash = 0x811c9dc5 >>> 0;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

function hsvToHex(hue: number): string {
  const s = 0.75, v = 1;
  const h = hue >= 1 ? hue - Math.floor(hue) : hue;
  const sextant = Math.floor(h * 6), f = h * 6 - sextant;
  const p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f));
  const [r = 0, g = 0, b = 0] = [[v, t, p], [q, v, p], [p, v, t], [p, q, v], [t, p, v], [v, p, q]][sextant % 6]!;
  return "#" + [r, g, b].map((c) => Math.max(0, Math.min(255, Math.round(c * 255))).toString(16).padStart(2, "0")).join("");
}

/** The name as the explorer shows it: letters and digits only, with `#<last 4>` when the name is not claimed. */
export function displayName(name: string | null | undefined, claimed: boolean, address?: string | null): string {
  let validated = "";
  for (const ch of name ?? "") if (ALPHANUMERIC.test(ch)) validated += ch;
  if (!validated || claimed || !address || address.length <= 4) return validated;
  return `${validated}#${address.slice(-4)}`;
}

export function avatarBackground(display: string): string {
  return display ? hsvToHex(fnv1a32(display) / 0xffffffff) : "#ffffff";
}

/** A coloured disc with the name's first letter, as a data URL, for a creator without a face. */
export function syntheticAvatar(name: string): string {
  const initial = (name.match(ALPHANUMERIC)?.[0] ?? "?").toUpperCase();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" fill="${avatarBackground(name)}"/><text x="16" y="22" text-anchor="middle" font-family="sans-serif" font-size="18" font-weight="700" fill="white">${initial}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
