import { z } from "zod";

import { ttlMemo } from "../ttl-memo";

export type DownloadOption = {
  os: string;
  osKey: OsKey;
  arch: ArchKey;
  archLabel: string;
  fileName: string;
  url: string;
};

type DownloadRelease = {
  version: string;
  options: Record<string, DownloadOption>;
};

const RELEASES_LATEST_API = "https://api.github.com/repos/decentraland/creator-hub/releases/latest";
const RELEASES_LATEST_PAGE = "https://github.com/decentraland/creator-hub/releases/latest";
// The links are rendered as-is, so only this repository's release downloads are accepted.
const RELEASE_DOWNLOAD_PREFIX = "https://github.com/decentraland/creator-hub/releases/download/";
// Unauthenticated GitHub API calls allow 60 an hour per IP, so each node reads the release at most every 10 minutes.
const RELEASE_TTL_MS = 10 * 60_000;
// A failed read is held for a minute, so a GitHub outage or rate limit does not repeat for each request.
const RELEASE_FAILURE_TTL_MS = 60_000;
// The download page waits for this read when the memo is cold.
const RELEASE_TIMEOUT_MS = 3_000;

const ReleaseSchema = z.object({
  tag_name: z.string(),
  assets: z.array(z.object({ name: z.string(), browser_download_url: z.url() })),
});

const ASSETS: { key: string; match: (name: string) => boolean; option: Omit<DownloadOption, "fileName" | "url"> }[] = [
  {
    key: "macos-arm64",
    match: (n) => n.includes("mac-arm64") && n.endsWith(".dmg"),
    option: { os: "macOS", osKey: "macos", arch: "arm64", archLabel: "Apple Silicon" },
  },
  {
    key: "macos-amd64",
    match: (n) => n.includes("mac-x64") && n.endsWith(".dmg"),
    option: { os: "macOS", osKey: "macos", arch: "amd64", archLabel: "Intel" },
  },
  {
    key: "windows-amd64",
    match: (n) => n.includes("win-x64") && n.endsWith(".exe"),
    option: { os: "Windows", osKey: "windows", arch: "amd64", archLabel: "x64" },
  },
];

const FALLBACK: DownloadRelease = {
  version: "latest",
  options: Object.fromEntries(
    ASSETS.map((a) => [a.key, { ...a.option, fileName: "", url: RELEASES_LATEST_PAGE }]),
  ),
};

const DEFAULT_OPTION = "macos-arm64";

function parseRelease(body: unknown): DownloadRelease | null {
  const parsed = ReleaseSchema.safeParse(body);
  if (!parsed.success) return null;
  const options: Record<string, DownloadOption> = {};
  // A release missing an installer would hand that system another one, so it counts as a failed read.
  for (const a of ASSETS) {
    const asset = parsed.data.assets.find((x) => a.match(x.name));
    if (!asset?.browser_download_url.startsWith(RELEASE_DOWNLOAD_PREFIX)) return null;
    options[a.key] = { ...a.option, fileName: asset.name, url: asset.browser_download_url };
  }
  return { version: parsed.data.tag_name, options };
}

async function readLatestRelease(): Promise<DownloadRelease | null> {
  const response = await fetch(RELEASES_LATEST_API, {
    signal: AbortSignal.timeout(RELEASE_TIMEOUT_MS),
    headers: { accept: "application/vnd.github+json" },
  });
  if (!response.ok) {
    await response.body?.cancel().catch(() => undefined);
    return null;
  }
  return parseRelease(await response.json());
}

type ReleaseRead = { release: DownloadRelease; read: boolean };

let lastRead: DownloadRelease | null = null;

async function fetchLatestRelease(): Promise<ReleaseRead> {
  const release = await readLatestRelease().catch(() => null);
  if (release === null) return { release: lastRead ?? FALLBACK, read: false };
  lastRead = release;
  return { release, read: true };
}

// Keyless: the release URL is a constant, so every visitor shares one entry for the life of the process.
const releaseMemo = ttlMemo({
  ttlMs: RELEASE_TTL_MS,
  ttlMsForValue: (r: ReleaseRead) => (r.read ? RELEASE_TTL_MS : RELEASE_FAILURE_TTL_MS),
  // An expired release serves while one reload runs behind it, so a visitor waits only when the memo is cold.
  staleMs: RELEASE_TTL_MS,
  load: fetchLatestRelease,
});

/** The latest published Creator Hub release; after a failed read the last one read, else links to the releases page. */
export async function loadLatestRelease(): Promise<DownloadRelease> {
  return (await releaseMemo()).release;
}

export function resetLatestRelease(): void {
  lastRead = null;
  releaseMemo.reset();
}

export type OsKey = "macos" | "windows";
type ArchKey = "arm64" | "amd64";

/** The desktop system the User-Agent names; phones, tablets and Linux have no Creator Hub installer. */
function detectOs(userAgent: string | null | undefined): OsKey | undefined {
  const ua = (userAgent ?? "").toLowerCase();
  if (/iphone|ipad|ipod|android|mobile/.test(ua)) return undefined;
  if (/windows|win64|win32|wow64/.test(ua)) return "windows";
  if (/mac os x|macintosh|mac_powerpc|darwin/.test(ua)) return "macos";
  return undefined;
}

function detectArch(userAgent: string | null | undefined, os: OsKey): ArchKey {
  const ua = (userAgent ?? "").toLowerCase();
  if (/arm64|aarch64/.test(ua)) return "arm64";
  if (os === "windows") return "amd64";
  if (/intel/.test(ua) && !/apple/.test(ua)) return "amd64";
  return os === "macos" ? "arm64" : "amd64";
}

function normalizeOs(v: string | null | undefined): OsKey | undefined {
  const s = (v ?? "").toLowerCase();
  if (s === "windows" || s === "win") return "windows";
  if (s === "macos" || s === "mac" || s === "osx" || s === "darwin") return "macos";
  return undefined;
}

function normalizeArch(v: string | null | undefined): ArchKey | undefined {
  const s = (v ?? "").toLowerCase();
  if (s === "arm64" || s === "aarch64" || s === "apple" || s === "silicon") return "arm64";
  if (s === "amd64" || s === "x64" || s === "x86_64" || s === "intel") return "amd64";
  return undefined;
}

type ResolvedDownload = {
  version: string;
  overridden: boolean;
  /** False when neither an override nor the User-Agent names Windows or macOS: the page offers no installer. */
  installable: boolean;
  detectedOs: OsKey;
  detectedArch: ArchKey;
  primary: DownloadOption;
  secondary: DownloadOption[];
};

function pickKey(
  options: Record<string, DownloadOption>,
  os: OsKey,
  arch: ArchKey,
): string | undefined {
  const exact = `${os}-${arch}`;
  if (options[exact]) return exact;
  const sameOs = Object.keys(options).find((k) => options[k].osKey === os);
  return sameOs;
}

export function resolveDownload(
  release: DownloadRelease,
  userAgent: string | null | undefined,
  override?: { os?: string | null; arch?: string | null },
): ResolvedDownload {
  const osOverride = normalizeOs(override?.os);
  const uaOs = detectOs(userAgent);
  const detectedOs = osOverride ?? uaOs ?? "macos";
  const archOverride = normalizeArch(override?.arch);
  const detectedArch = archOverride ?? detectArch(userAgent, detectedOs);

  const options = release.options;
  const primaryKey = pickKey(options, detectedOs, detectedArch) ?? DEFAULT_OPTION;
  const primary = options[primaryKey] ?? FALLBACK.options[DEFAULT_OPTION];

  const secondary: DownloadOption[] = [];
  const seenOs = new Set<OsKey>([primary.osKey]);
  for (const key of Object.keys(options)) {
    const opt = options[key];
    if (seenOs.has(opt.osKey)) continue;
    const altKey = pickKey(options, opt.osKey, detectedArch);
    const chosen = (altKey && options[altKey]) || opt;
    secondary.push(chosen);
    seenOs.add(opt.osKey);
  }

  return {
    version: release.version,
    overridden: osOverride !== undefined || archOverride !== undefined,
    installable: (osOverride ?? uaOs) !== undefined,
    detectedOs,
    detectedArch,
    primary,
    secondary,
  };
}
