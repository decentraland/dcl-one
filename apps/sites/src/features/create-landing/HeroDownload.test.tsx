// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DownloadOption, HeroDownloads } from "@/data/lib/download/creator-hub.server";
import { detectMacChip } from "@/data/lib/download/mac-chip";
import { track } from "@/core/lib/telemetry/track";

import HeroDownload from "./HeroDownload";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  });
});
// A document whose analytics is ready: its page view has been sent.
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));
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

const onMac: HeroDownloads = { primary: appleSilicon, others: [intel, windows] };
const onWindows: HeroDownloads = { primary: windows, others: [appleSilicon, intel] };

const assign = vi.fn();

beforeEach(() => {
  vi.mocked(detectMacChip).mockReturnValue("unknown");
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.stubGlobal("location", { href: "http://localhost/", assign });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

const main = () => screen.getByRole("link", { name: /Download Creator Hub/ });
const user = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

it("links to the download page when there is no installer to offer in place", () => {
  render(<HeroDownload downloads={null} sid="s1" />);
  expect(main()).toHaveProperty("href", expect.stringMatching(/\/download\/creator-hub$/));
  expect(screen.queryByText("Also available on")).toBeNull();
});

it("offers the installer for the visitor's system, and the other builds under it", () => {
  render(<HeroDownload downloads={onWindows} sid="s1" />);
  expect(main()).toHaveProperty("href", windows.url);
  expect(main().getAttribute("title")).toBeNull();
  expect(screen.getAllByRole("tooltip").map((tip) => tip.textContent)).toContain("Download for Windows");
  expect(screen.getByRole("link", { name: "Download for macOS (Apple Silicon)" })).toHaveProperty("href", appleSilicon.url);
  expect(screen.getByRole("link", { name: "Download for macOS (Intel)" })).toHaveProperty("href", intel.url);
});

it("puts the Intel build first on a Mac the browser can tell is an Intel one", () => {
  vi.mocked(detectMacChip).mockReturnValue("intel");
  render(<HeroDownload downloads={onMac} sid="s1" />);
  expect(main()).toHaveProperty("href", intel.url);
  expect(screen.getByRole("link", { name: "Download for macOS (Apple Silicon)" })).toHaveProperty("href", appleSilicon.url);
});

it("keeps the server's pick when the browser cannot tell the Mac's chip", () => {
  render(<HeroDownload downloads={onMac} sid="s1" />);
  expect(main()).toHaveProperty("href", appleSilicon.url);
});

it("links an iPad, which names itself a Mac, to the download page", () => {
  vi.stubGlobal("navigator", { ...window.navigator, maxTouchPoints: 5 });
  render(<HeroDownload downloads={onMac} sid="s1" />);
  expect(main()).toHaveProperty("href", expect.stringMatching(/\/download\/creator-hub$/));
  expect(screen.queryByText("Also available on")).toBeNull();
});

it("reports the download and shows the install steps three seconds later", async () => {
  render(<HeroDownload downloads={onWindows} sid="s1" />);
  await user().click(main());
  expect(track).toHaveBeenCalledWith(
    "lp_creatorhub_download_clicked",
    { os: "windows", arch: "amd64", file_name: "hub-win-x64.exe", kind: "primary" },
    { sid: "s1", story: "landings/creator-hub-download" },
  );
  expect(assign).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(3000));
  expect(assign).toHaveBeenCalledWith("/download/creator-hub-success?os=windows&arch=amd64");
});

it("reports another build as a secondary download and shows the steps for that build", async () => {
  render(<HeroDownload downloads={onWindows} sid="s1" />);
  await user().click(screen.getByRole("link", { name: "Download for macOS (Intel)" }));
  expect(track).toHaveBeenCalledWith(
    "lp_creatorhub_download_clicked",
    expect.objectContaining({ os: "macos", arch: "amd64", kind: "secondary" }),
    expect.anything(),
  );
  act(() => vi.advanceTimersByTime(3000));
  expect(assign).toHaveBeenCalledWith("/download/creator-hub-success?os=macos&arch=amd64");
});

it("stays on the page when the download is opened in another tab", async () => {
  render(<HeroDownload downloads={onWindows} sid="s1" />);
  const u = user();
  await u.keyboard("{Meta>}");
  await u.click(main());
  await u.keyboard("{/Meta}");
  act(() => vi.advanceTimersByTime(3000));
  expect(track).toHaveBeenCalled();
  expect(assign).not.toHaveBeenCalled();
});

it("does not leave for the install steps once the visitor has left the landing", async () => {
  const { unmount } = render(<HeroDownload downloads={onWindows} sid="s1" />);
  await user().click(main());
  unmount();
  act(() => vi.advanceTimersByTime(3000));
  expect(assign).not.toHaveBeenCalled();
});

describe("legacy Click (inventory 2.1.a)", () => {
  const clicks = () => vi.mocked(track).mock.calls.filter(([event]) => event === "Click");

  it("row 28: the main button sends Creators Hero / creator_hub, a conversion with no correlation and no os", async () => {
    sessionStorage.clear();
    render(<HeroDownload downloads={onWindows} sid="s1" />);
    await user().click(main());
    expect(clicks()).toHaveLength(1);
    const [, props, ctx] = clicks()[0]!;
    expect(props).toMatchObject({ place: "Creators Hero", event: "Download", download_target: "creator_hub" });
    expect(props).not.toHaveProperty("os");
    expect(props).not.toHaveProperty("click_id");
    expect(sessionStorage.getItem("downloadFunnel:lastClick")).toBeNull();
    expect(ctx).toEqual({ sid: "s1", legacy: true, conversion: true });
  });

  it("row 29: another build names its system", async () => {
    render(<HeroDownload downloads={onWindows} sid="s1" />);
    await user().click(screen.getByRole("link", { name: "Download for macOS (Intel)" }));
    expect(clicks()[0]![1]).toMatchObject({ place: "Creators Hero", event: "Download", download_target: "creator_hub", os: "macOS" });
  });

  it("row 30: the link to the download page, while no installer is offered in place", async () => {
    render(<HeroDownload downloads={null} sid="s1" />);
    main().addEventListener("click", (e) => e.preventDefault());
    await user().click(main());
    expect(track).toHaveBeenCalledTimes(1);
    const [, props, ctx] = clicks()[0]!;
    expect(props).toMatchObject({ place: "Creators Hero", event: "Download", download_target: "creator_hub" });
    expect(props).not.toHaveProperty("os");
    expect(props).not.toHaveProperty("click_id");
    expect(ctx).toEqual({ sid: "s1", legacy: true, conversion: true });
  });

  it("keeps lp_creatorhub_download_clicked unchanged beside it", async () => {
    render(<HeroDownload downloads={onWindows} sid="s1" />);
    await user().click(main());
    expect(vi.mocked(track).mock.calls.map(([event]) => event)).toEqual(["lp_creatorhub_download_clicked", "Click"]);
    expect(vi.mocked(track).mock.calls[0]).toEqual([
      "lp_creatorhub_download_clicked",
      { os: "windows", arch: "amd64", file_name: "hub-win-x64.exe", kind: "primary" },
      { sid: "s1", story: "landings/creator-hub-download" },
    ]);
  });
});
