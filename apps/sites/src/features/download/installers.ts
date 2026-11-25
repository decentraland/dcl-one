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
