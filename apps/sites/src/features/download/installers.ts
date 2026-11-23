import { collectCampaignParams } from "@/core/lib/telemetry/campaign-params";

export type InstallerOs = "windows" | "macos";
export type InstallerArch = "amd64" | "arm64";

export function installerSelection(params: URLSearchParams): {
  os: InstallerOs;
  arch: InstallerArch;
} {
  const os =
    params.get("os")?.toLowerCase() === "windows" ? "windows" : "macos";
  const raw = params.get("arch");
  const arch =
    raw === "amd64" || raw === "arm64"
      ? raw
      : os === "windows"
        ? "amd64"
        : "arm64";
  return { os, arch };
}

export function downloadSuccessHref(option: {
  os: InstallerOs;
  arch: InstallerArch;
}): string {
  return `/download_success?${new URLSearchParams(option)}`;
}

export function canonicalSuccessHref(request: Request): string {
  return `/download_success${new URL(request.url).search}`;
}

/**
 * The legacy site's `/download_success` link (`buildDownloadSuccessHref`): `os` and the control's `place`, then the
 * visitor's anonymous id, the architecture (only /download sent it) and the linking page's `utm_*`, in that order.
 */
export function legacySuccessHref(
  os: InstallerOs,
  place: string,
  { anonUserId, arch, search = "" }: { anonUserId?: string; arch?: InstallerArch; search?: string } = {},
): string {
  const params = new URLSearchParams({ os: os === "windows" ? "Windows" : "macOS", place });
  if (anonUserId) params.set("anon_user_id", anonUserId);
  if (arch) params.set("arch", arch);
  for (const [key, value] of Object.entries(collectCampaignParams(search))) {
    if (!params.has(key)) params.set(key, value);
  }
  return `/download_success?${params}`;
}

/** An internal path carrying the linking page's `utm_*`, so a partner-attributed visit keeps them. */
export function withCampaignParams(path: string, search = ""): string {
  const collected = new URLSearchParams(collectCampaignParams(search));
  if (collected.size === 0) return path;
  return `${path}${path.includes("?") ? "&" : "?"}${collected}`;
}

/** Only the `utm_*` of `search`, normalized: what a loader hands its page for the links above. */
export function campaignSearch(search: string): string {
  return new URLSearchParams(collectCampaignParams(search)).toString();
}
