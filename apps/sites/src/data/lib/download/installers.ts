import { explorerInstallersUrl } from "../catalyst/bases";

export type InstallerOs = "windows" | "macos";
export type InstallerArch = "amd64" | "arm64";
/** `url` is null when the deployment configures no installer host (EXPLORER_INSTALLERS_URL). */
export type Installer = { os: InstallerOs; arch: InstallerArch; url: string | null };

type Links = { windows: { amd64: string | null }; macos: { amd64: string | null; arm64: string | null } };

// from: decentraland-ui2 src/modules/cdnReleases.ts, CDNSource.LAUNCHER: each installer's path on the launcher host.
const PATHS = {
  windows: { amd64: "/launcher-rust/Decentraland_installer.exe" },
  macos: { amd64: "/launcher/dcl/Decentraland%20Outdated-mac-x64.dmg", arm64: "/launcher-rust/Decentraland_installer.dmg" },
};

/** The installers under the configured launcher host; the node serves none, so without one every link is null. */
function installerLinks(): Links {
  const base = explorerInstallersUrl();
  const at = (path: string) => (base ? `${base}${path}` : null);
  return { windows: { amd64: at(PATHS.windows.amd64) }, macos: { amd64: at(PATHS.macos.amd64), arm64: at(PATHS.macos.arm64) } };
}

type Agent = { os: InstallerOs | "other"; handheld: boolean };

function readAgent(userAgent: string): Agent {
  const ua = userAgent.toLowerCase();
  const handheld = /iphone|ipad|ipod|android|mobile/.test(ua);
  const os = handheld ? "other" : /windows/.test(ua) ? "windows" : /macintosh|mac os x/.test(ua) ? "macos" : "other";
  return { os, handheld };
}

// from: sites@17b3535a:src/modules/downloadLinks.ts resolveInstaller: macOS takes Apple Silicon first; Windows its
// exact architecture, else x64 when the architecture is unknown or arm64; Windows has only an x64 installer.
function resolveInstaller(os: InstallerOs, links: Links): Installer {
  if (os === "macos") return { os, arch: "arm64", url: links.macos.arm64 };
  return { os, arch: "amd64", url: links.windows.amd64 };
}

/**
 * The /download buttons for a visitor's user agent.
 * from: sites@17b3535a:src/components/DownloadOptions/DownloadOptions.tsx: Windows and macOS get their own installer
 * and the other one as the secondary icon; any other desktop system gets Windows with macOS secondary; phones and
 * tablets get no installer.
 */
export function downloadOptions(userAgent: string): { primary: Installer | null; secondary: Installer } {
  const agent = readAgent(userAgent);
  const links = installerLinks();
  const os = agent.os === "other" ? "windows" : agent.os;
  return { primary: agent.handheld ? null : resolveInstaller(os, links), secondary: resolveInstaller(os === "macos" ? "windows" : "macos", links) };
}

export function successHref(installer: Pick<Installer, "os" | "arch">): string {
  return `/download_success?os=${installer.os === "windows" ? "Windows" : "macOS"}&arch=${installer.arch}`;
}

