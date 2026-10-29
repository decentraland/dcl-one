type SiteWindow = Window & typeof globalThis & { __SITE_BASE__?: string };

// A server render has no window: it uses the node's public URL, the same origin
// the browser hydrates with, or stays relative when that is not configured.
function serverBase(): string {
  const proc = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process;
  return proc?.env?.PUBLIC_CATALYST_URL || "";
}

export function siteBase(override?: string): string {
  const fromEnv: string | undefined =
    typeof import.meta !== "undefined" ? import.meta.env?.VITE_SITE_URL : undefined;
  const fromWindow =
    typeof window !== "undefined" ? (window as SiteWindow).__SITE_BASE__ : undefined;
  return (override ?? fromEnv ?? fromWindow ?? (typeof window !== "undefined" ? window.location.origin : serverBase())).replace(/\/$/, "");
}

export function siteUrl(path = ""): string {
  if (!path) return siteBase();
  return siteBase() + (path.startsWith("/") ? path : `/${path}`);
}

export function siteHost(): string {
  const base = siteBase();
  return base ? new URL(base).host : "";
}
