import type { DownloadArch, DownloadOs } from "./params";

// Static launcher builds (ui2@2cca0cf3 cdnReleases). The CDN serves a generic installer: no identity,
// no attribution, no deep link. macOS amd64 has no gateway build, so it is always the legacy DMG.
const LAUNCHER_BASE =
  "https://explorer-artifacts.decentraland.org/launcher-rust";
const LEGACY_MAC_INTEL =
  "https://explorer-artifacts.decentraland.org/launcher/dcl/Decentraland%20Outdated-mac-x64.dmg";

export function cdnInstallerUrl(os: DownloadOs, arch: DownloadArch): string {
  if (os === "windows") return `${LAUNCHER_BASE}/Decentraland_installer.exe`;
  return arch === "amd64"
    ? LEGACY_MAC_INTEL
    : `${LAUNCHER_BASE}/Decentraland_installer.dmg`;
}

export type InstallerSource = "identity" | "anonymous" | "cdn";

export type InstallerUrl = {
  url: string;
  source: InstallerSource;
  filename: string;
};

/**
 * Behavior 4a: the identity gateway URL, else the anonymous gateway URL, else the static CDN only
 * when no gateway URL can be resolved: DOWNLOAD_GATEWAY_URL unset or blank, or the macOS amd64
 * legacy build, which the gateway does not sign. With a gateway configured the CDN is never chosen,
 * so the deep link and referrer always reach the signed binary; `anonUserId` is required for that
 * (callers mint one when the visitor has none). Attribution rides the gateway URL's query.
 */
export function resolveInstallerUrl(input: {
  os: DownloadOs;
  arch: DownloadArch;
  gateway?: string;
  identityId?: string;
  anonUserId: string;
  query?: Record<string, string | undefined>;
}): InstallerUrl {
  const extension = input.os === "windows" ? "exe" : "dmg";
  const gateway = input.gateway?.trim().replace(/\/+$/, "");
  const gatewayBuild = input.os === "windows" || input.arch === "arm64";
  let base: string;
  let source: InstallerSource;
  if (gateway && gatewayBuild && input.identityId) {
    base = `${gateway}/${encodeURIComponent(input.identityId)}/decentraland.${extension}`;
    source = "identity";
  } else if (gateway && gatewayBuild) {
    base = `${gateway}/anonymous/decentraland.${extension}`;
    source = "anonymous";
  } else {
    base = cdnInstallerUrl(input.os, input.arch);
    source = "cdn";
  }
  const url = new URL(base);
  // Attribution only means something to the gateway, which bakes it into the signed binary; the
  // CDN serves a generic installer. Never a UUID-shaped click id here: macOS launchers read the
  // identity from this URL (contract §3).
  if (source !== "cdn")
    for (const [key, value] of Object.entries({
      anon_user_id: input.anonUserId,
      ...input.query,
    }))
      if (value) url.searchParams.append(key, value);
  const filename =
    source === "cdn" ? filenameOf(base) : `decentraland.${extension}`;
  return { url: url.toString(), source, filename };
}

function filenameOf(url: string): string {
  const last = decodeURIComponent(new URL(url).pathname.split("/").pop() ?? "");
  return last.includes(".") ? last : "launcher";
}
