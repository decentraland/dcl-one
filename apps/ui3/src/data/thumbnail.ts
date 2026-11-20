import { catalystBase } from "./catalyst/client";

export { useThumbnailPublicBase } from "@dcl/one-ui/components/AppChrome/thumbnail-public-base";

export type ThumbnailWidth = 80 | 160 | 320 | 640 | 960;

function catalystOrigin(): string | null {
  try {
    return new URL(catalystBase()).origin;
  } catch {
    return null;
  }
}

type EmbeddedWindow = Window & typeof globalThis & {
  __CATALYST_BASE__?: string;
  __DCL_PLAY_EMBEDDED__?: boolean;
  __SITE_BASE__?: string;
};

const POSTER_MOUNT = "events-assets-099ac00";
const POSTER_BUCKET = `${POSTER_MOUNT}.decentraland.org`;
const UPLOADED_POSTER = /^\/[^/]+\/[0-9a-f]{64}\.[a-z]+$/i;

function embeddedSiteBase(): string {
  if (typeof window === "undefined") return "";
  const w = window as EmbeddedWindow;
  return w.__DCL_PLAY_EMBEDDED__ && typeof w.__SITE_BASE__ === "string" ? w.__SITE_BASE__.replace(/\/$/, "") : "";
}

function mediaBase(): string {
  return (import.meta.env.VITE_CATALYST_URL ?? "").replace(/\/$/, "") || embeddedSiteBase();
}

function parseBase(value: string | undefined): URL | undefined {
  if (!value) return undefined;
  try {
    return new URL(value);
  } catch {
    return undefined;
  }
}

function mountedPoster(url: URL, publicBase: string | undefined): { path: string; converterBase: string } | undefined {
  // The SSR transport can use a different origin than the public page.
  const renderedBase = import.meta.env.SSR
    ? (typeof process === "undefined" ? undefined : process.env.PUBLIC_CATALYST_URL)
    : (typeof window === "undefined" ? undefined : (window as EmbeddedWindow).__CATALYST_BASE__);
  const base = parseBase(mediaBase()) ?? parseBase(renderedBase || publicBase);
  if (!base) return undefined;
  if (url.origin !== base.origin) return undefined;
  const prefix = base.pathname.replace(/\/+$/, "");
  const mount = `${prefix}/${POSTER_MOUNT}`;
  if (!url.pathname.startsWith(mount)) return undefined;
  const path = url.pathname.slice(mount.length);
  return /^\/poster(?:-vertical)?\/[^/]+$/.test(path) ? { path, converterBase: base.origin + prefix } : undefined;
}

function resized(source: string, width: ThumbnailWidth, base: string): string {
  return `${base}/media/convert?width=${width}&url=${encodeURIComponent(source)}`;
}

export function publicThumbnail(source: string | null | undefined, width: ThumbnailWidth = 640, publicBase?: string): string | undefined {
  if (!source) return undefined;
  if (source.startsWith("/")) return source;
  try {
    const url = new URL(source);
    if (url.protocol !== "https:" || url.search || url.username || url.password || /\.(gif|svg)$/i.test(url.pathname)) return source;
    const catalyst = catalystOrigin();
    if (catalyst !== null && url.origin === catalyst && /^\/lambdas\/collections\/contents\/[^/]+\/thumbnail$/.test(url.pathname)) {
      url.searchParams.set("width", String(width));
      return url.toString();
    }
    const mounted = mountedPoster(url, publicBase);
    if (mounted) {
      return UPLOADED_POSTER.test(mounted.path)
        ? source
        : resized(`https://${POSTER_BUCKET}${mounted.path}`, width, mediaBase() || mounted.converterBase);
    }
    const known = url.hostname === POSTER_BUCKET
      || /^(peer(?:-[a-z]+\d)?|worlds-content-server|assets-cdn)\.decentraland\.org$/.test(url.hostname);
    if (/^peer(?:-[a-z]+\d)?\.decentraland\.org$/.test(url.hostname) && /^\/lambdas\/collections\/contents\/[^/]+\/thumbnail$/.test(url.pathname)) {
      return `${mediaBase()}${url.pathname}?width=${width}`;
    }
    const posterCopy = !known && url.hostname.startsWith(`${POSTER_MOUNT}.`)
      && /^\/poster(?:-vertical)?\/[^/]+$/.test(url.pathname) && !UPLOADED_POSTER.test(url.pathname);
    if (posterCopy) source = `https://${POSTER_BUCKET}${url.pathname}`;
    else if (!known) return source;
    return resized(source, width, mediaBase());
  } catch { return source; }
}

export function nodeMapImage(source: string | null | undefined): string | undefined {
  if (!source) return undefined;
  try {
    const url = new URL(source);
    if (url.hostname !== "api.decentraland.org" || !/^\/v[12]\/(?:parcels\/-?\d+\/-?\d+|estates\/\d+)\/map\.png$/.test(url.pathname)) return source;
    return `${mediaBase()}/api${url.pathname}${url.search}`;
  } catch { return source; }
}

export function publicThumbnailSet(source: string | null | undefined, widths: readonly ThumbnailWidth[], publicBase?: string): string | undefined {
  if (!source || !widths.length || publicThumbnail(source, widths[0], publicBase) === source) return undefined;
  return widths.map(width => `${publicThumbnail(source, width, publicBase)} ${width}w`).join(", ");
}
