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

/** The current Sites native-artifact contract; URL input never supplies a download host. */
export function installerUrl(os: InstallerOs, arch: InstallerArch): string {
  if (os === "macos" && arch === "amd64")
    return "https://explorer-artifacts.decentraland.org/launcher/dcl/Decentraland%20Outdated-mac-x64.dmg";
  return `https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.${os === "windows" ? "exe" : "dmg"}`;
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
