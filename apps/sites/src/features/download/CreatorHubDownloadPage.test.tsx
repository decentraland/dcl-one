// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps, ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { DownloadOption } from "@/data/lib/download/creator-hub.server";
import { detectMacChip } from "@/data/lib/download/mac-chip";
import { track } from "@/core/lib/telemetry/track";

import CreatorHubDownloadPage from "./CreatorHubDownloadPage";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/download/mac-chip", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/download/mac-chip")>()),
  detectMacChip: vi.fn(() => "unknown"),
}));

const BASE = "https://github.com/decentraland/creator-hub/releases/download/1.2.3";
const option = (osKey: "macos" | "windows", arch: "arm64" | "amd64", archLabel: string, fileName: string): DownloadOption => ({
  os: osKey === "macos" ? "macOS" : "Windows",
  osKey,
  arch,
  archLabel,
  fileName,
  url: `${BASE}/${fileName}`,
});
const appleSilicon = option("macos", "arm64", "Apple Silicon", "hub-mac-arm64.dmg");
const intel = option("macos", "amd64", "Intel", "hub-mac-x64.dmg");
const windows = option("windows", "amd64", "x64", "hub-win-x64.exe");

const onMac: ComponentProps<typeof CreatorHubDownloadPage> = {
  sid: "s1",
  version: "1.2.3",
  overridden: false,
  archChosen: false,
  detectedOs: "macos",
  detectedArch: "arm64",
  primary: appleSilicon,
  secondary: [intel, windows],
};

const assign = vi.fn();
const user = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
const main = () => screen.getByRole("link", { name: "Download for macOS" });

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.stubGlobal("location", { href: "http://localhost/", assign });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

it("offers the installer for this system and every other build, Mac builds named by their chip", () => {
  render(<CreatorHubDownloadPage {...onMac} />);
  expect(main()).toHaveProperty("href", appleSilicon.url);
  expect(screen.getByRole("link", { name: "Download for macOS (Intel)" })).toHaveProperty("href", intel.url);
  expect(screen.getByRole("link", { name: "Download for Windows" })).toHaveProperty("href", windows.url);
});

it("puts the Intel build first on an Intel Mac, and reports the page and the download for that build", async () => {
  vi.mocked(detectMacChip).mockReturnValue("intel");
  render(<CreatorHubDownloadPage {...onMac} />);
  expect(main()).toHaveProperty("href", intel.url);
  expect(screen.getByRole("link", { name: "Download for macOS (Apple Silicon)" })).toHaveProperty("href", appleSilicon.url);
  expect(track).toHaveBeenCalledWith("lp_creatorhub_download_viewed", expect.objectContaining({ os: "macos", arch: "amd64" }), expect.anything());
  expect(track).not.toHaveBeenCalledWith("lp_creatorhub_download_viewed", expect.objectContaining({ arch: "arm64" }), expect.anything());

  await user().click(main());
  expect(track).toHaveBeenCalledWith(
    "lp_creatorhub_download_clicked",
    expect.objectContaining({ os: "macos", arch: "amd64", kind: "primary" }),
    expect.anything(),
  );
});

it("keeps the chip a link names first", () => {
  vi.mocked(detectMacChip).mockReturnValue("intel");
  render(<CreatorHubDownloadPage {...onMac} overridden archChosen />);
  expect(main()).toHaveProperty("href", appleSilicon.url);
});

it("still picks the build for this Mac when a link names only the system", () => {
  vi.mocked(detectMacChip).mockReturnValue("intel");
  render(<CreatorHubDownloadPage {...onMac} overridden />);
  expect(main()).toHaveProperty("href", intel.url);
});

it("shows the install steps three seconds after a download", async () => {
  render(<CreatorHubDownloadPage {...onMac} />);
  await user().click(screen.getByRole("link", { name: "Download for Windows" }));
  act(() => vi.advanceTimersByTime(3000));
  expect(assign).toHaveBeenCalledWith("/download/creator-hub-success?os=windows&arch=amd64");
});

it("does not move to the install steps once the page was left for another", async () => {
  render(<CreatorHubDownloadPage {...onMac} />);
  await user().click(main());
  act(() => {
    window.dispatchEvent(new Event("pagehide"));
    vi.advanceTimersByTime(3000);
  });
  expect(assign).not.toHaveBeenCalled();
});

it("offers no download on a system without an installer", () => {
  render(<CreatorHubDownloadPage {...onMac} detectedOs="macos" primary={null} secondary={[]} />);
  expect(screen.getByRole("heading", { name: "Decentraland Creator Hub" })).toBeTruthy();
  expect(screen.queryByRole("link", { name: /^Download for/ })).toBeNull();
});
