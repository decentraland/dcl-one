import { catalystBase } from "./catalyst/client";

export type ThumbnailWidth = 80 | 160 | 320 | 640 | 960;

function catalystOrigin(): string | null {
  try {
    return new URL(catalystBase()).origin;
  } catch {
    return null;
  }
}

type EmbeddedWindow = Window & typeof globalThis & { __DCL_PLAY_EMBEDDED__?: boolean; __SITE_BASE__?: string };

function embeddedSiteBase(): string {
  if (typeof window === "undefined") return "";
  const w = window as EmbeddedWindow;
  return w.__DCL_PLAY_EMBEDDED__ && typeof w.__SITE_BASE__ === "string" ? w.__SITE_BASE__.replace(/\/$/, "") : "";
}

function mediaBase(): string {
  return (import.meta.env.VITE_CATALYST_URL ?? "").replace(/\/$/, "") || embeddedSiteBase();
}

export function publicThumbnail(source: string | null | undefined, width: ThumbnailWidth = 640): string | undefined {
  if (!source) return undefined;
  try {
    const url = new URL(source);
    if (url.protocol !== "https:" || url.search || url.username || url.password || /\.(gif|svg)$/i.test(url.pathname)) return source;
    const catalyst = catalystOrigin();
    if (catalyst !== null && url.origin === catalyst && /^\/lambdas\/collections\/contents\/[^/]+\/thumbnail$/.test(url.pathname)) {
      url.searchParams.set("width", String(width));
      return url.toString();
    }
    const known = /^(peer(?:-[a-z]+\d)?|worlds-content-server|events-assets-099ac00|assets-cdn)\.decentraland\.org$/.test(url.hostname);
    if (/^peer(?:-[a-z]+\d)?\.decentraland\.org$/.test(url.hostname) && /^\/lambdas\/collections\/contents\/[^/]+\/thumbnail$/.test(url.pathname)) {
      return `${mediaBase()}${url.pathname}?width=${width}`;
    }
    const eventMirror = url.hostname === "events-assets-099ac00.interconnected.online"
      && /^\/poster(?:-vertical)?\/[^/]+$/.test(url.pathname);
    const posterCopy = !known && !eventMirror && url.hostname.startsWith("events-assets-099ac00.")
      && /^\/poster(?:-vertical)?\/[^/]+$/.test(url.pathname) && !/^\/[^/]+\/[0-9a-f]{64}\.[a-z]+$/i.test(url.pathname);
    if (posterCopy) source = `https://events-assets-099ac00.decentraland.org${url.pathname}`;
    else if (!known && !eventMirror) return source;
    return `${mediaBase()}/media/convert?width=${width}&url=${encodeURIComponent(source)}`;
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

export function publicThumbnailSet(source: string | null | undefined, widths: readonly ThumbnailWidth[]): string | undefined {
  if (!source || !widths.length || publicThumbnail(source, widths[0]) === source) return undefined;
  return widths.map(width => `${publicThumbnail(source, width)} ${width}w`).join(", ");
}
