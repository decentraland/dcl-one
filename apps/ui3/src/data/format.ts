export function truncateAddress(address: string | null | undefined): string {
  if (!address) return "";
  return address.length > 12
    ? `${address.slice(0, 6)}\u{2026}${address.slice(-4)}`
    : address;
}

/** Shortens hex addresses; any other label (a name) passes through unchanged. */
export function truncateIfAddress(value: string): string {
  return /^0x[0-9a-fA-F]{6,}$/.test(value) ? truncateAddress(value) : value;
}

export function hueFor(seed: unknown): number {
  const s = String(seed ?? "");
  let h = 0;
  for (let i = 0; i < s.length; i += 1) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

export function isValidAddress(addr: string | null | undefined): boolean {
  return ADDRESS_RE.test((addr ?? "").trim());
}

/** "3 days ago", "1 year ago": how long since `iso`, in the largest whole unit; null for
    nothing or an unreadable date. */
export function timeAgo(iso: string | null | undefined, now: number = Date.now()): string | null {
  if (!iso) return null;
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return null;
  const s = Math.max(0, Math.floor((now - t) / 1000));
  const units: [number, string][] = [[31536000, "year"], [2592000, "month"], [604800, "week"], [86400, "day"], [3600, "hour"], [60, "minute"]];
  for (const [span, name] of units) {
    if (s >= span) { const n = Math.floor(s / span); return `${n} ${name}${n === 1 ? "" : "s"} ago`; }
  }
  return "just now";
}
