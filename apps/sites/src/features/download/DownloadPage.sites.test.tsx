// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";

import { trackDownloadClick } from "./download-click";
import DownloadPage from "./DownloadPage";
import { googlePlayHref } from "./google-play";
import sitesLogo from "@dcl/one-ui/apps/sites/assets/dcl-logo.svg?url";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@dcl/one-ui/components/AppChrome/chrome-auth", () => ({ useChromeAuth: () => ({ signedIn: false, account: null, name: null, onSignIn: () => {} }) }));

// A document whose analytics is ready: its page view has been sent.
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));
vi.mock("@/data/lib/download/mac-chip", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/download/mac-chip")>()),
  detectMacChip: vi.fn(() => "intel"),
}));

afterEach(() => {
  cleanup();
  vi.mocked(track).mockReset();
});

it("links home through the decorative 48px Decentraland logo", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" />);
  const home = screen.getAllByRole("link", { name: "Decentraland" }).find((a) => a.getAttribute("href") === "/");
  const logo = home!.querySelector("img");
  expect(logo!.getAttribute("alt")).toBe("");
  expect([logo!.getAttribute("width"), logo!.getAttribute("height")]).toEqual(["48", "48"]);
  expect(logo!.getAttribute("src")).toBe(sitesLogo);
});

const WINDOWS = { os: "windows", arch: "amd64", url: "https://installers.test/Decentraland_installer.exe" } as const;
const MACOS = { os: "macos", arch: "arm64", url: "https://installers.test/Decentraland_installer.dmg" } as const;

it("names the page with level 1 headings only, one for each layout", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads="+438K" sid="s1" />);
  const headings = screen.getAllByRole("heading");
  expect(headings.map((h) => h.tagName)).toEqual(["H1", "H1"]);
  expect(new Set(headings.map((h) => h.textContent)).size).toBe(1);
});

it("links each installer button to the page that starts that installer, a click only reporting itself", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" />);
  expect(screen.getByRole("link", { name: "Download for Windows" }).getAttribute("href")).toBe("/download_success?os=Windows&place=download-page&arch=amd64");
  expect(screen.getByRole("link", { name: "macOS" }).getAttribute("href")).toBe("/download_success?os=macOS&place=download-page&arch=arm64");
});

it("carries a UUID visitor id and the page's utm_* on those links, in the legacy order", () => {
  const anon = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid={anon} search="utm_source=partner" />);
  expect(screen.getByRole("link", { name: "Download for Windows" }).getAttribute("href")).toBe(
    `/download_success?os=Windows&place=download-page&anon_user_id=${anon}&arch=amd64&utm_source=partner`,
  );
});

describe("legacy download Clicks (inventory 2.1.a)", () => {
  const clickOf = async (name: string | RegExp, android = false) => {
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={android} downloads={null} sid="s1" />);
    const link = screen.getAllByRole("link", { name }).at(-1)!;
    // A link with no handler of its own would navigate jsdom away.
    link.addEventListener("click", (e) => e.preventDefault());
    await userEvent.setup().click(link);
    expect(track).toHaveBeenCalledTimes(1);
    const [event, props, ctx] = vi.mocked(track).mock.calls[0]!;
    expect(event).toBe("Click");
    expect(ctx).toEqual({ sid: "s1", legacy: true, conversion: true });
    return props as Record<string, unknown>;
  };

  it("row 18: the main installer button is place Download, with no os", async () => {
    const props = await clickOf("Download for Windows");
    expect(props).toMatchObject({ place: "Download", event: "Download", download_target: "desktop_installer" });
    expect(props).not.toHaveProperty("os");
    expect(props.click_id).toEqual(expect.any(String));
  });

  it("row 19: Epic", async () => {
    expect(await clickOf("Download on Epic Games")).toMatchObject({ place: "download-page", event: "Download", download_target: "epic" });
  });

  it("row 20: the other system's installer, with no os", async () => {
    const props = await clickOf("macOS");
    expect(props).toMatchObject({ place: "download-page", event: "Download", download_target: "desktop_installer" });
    expect(props).not.toHaveProperty("os");
  });

  it("row 21: the iOS icon", async () => {
    expect(await clickOf("iOS")).toMatchObject({ place: "download-page", event: "Download", download_target: "app_store", os: "iOS" });
  });

  it("row 22: the Google Play icon", async () => {
    expect(await clickOf("Google Play")).toMatchObject({ place: "download-page", event: "Download", download_target: "google_play", os: "Android" });
  });

  it("row 23: the phone layout's Google Play badge on Android", async () => {
    expect(await clickOf(/Get it on Google Play/, true)).toMatchObject({
      place: "download-page",
      event: "Download",
      download_target: "google_play",
      os: "Android",
    });
  });

  it("row 24: the phone layout's App Store badge elsewhere", async () => {
    expect(await clickOf(/Download on the App Store/)).toMatchObject({
      place: "download-page",
      event: "Download",
      download_target: "app_store",
      os: "iOS",
    });
  });
});

describe("the Google Play departure", () => {
  it("tags both Google Play controls with the visitor's campaign over the site's defaults", () => {
    const googlePlay = googlePlayHref("?utm_source=Partner&utm_campaign=Spring%20Drop");
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android downloads={null} sid="s1" googlePlay={googlePlay} />);
    const hrefs = [
      screen.getByRole("link", { name: "Google Play" }).getAttribute("href"),
      screen.getByRole("link", { name: /Get it on Google Play/ }).getAttribute("href"),
    ];
    for (const href of hrefs) {
      const params = new URL(href!).searchParams;
      expect(Object.fromEntries(params)).toMatchObject({
        utm_org: "dclrgl",
        utm_source: "partner",
        utm_medium: "qr",
        utm_campaign: "spring_drop",
        utm_content: "android",
        referrer: "utm_org=dclrgl&utm_source=partner&utm_medium=qr&utm_campaign=spring_drop&utm_content=android",
      });
    }
  });

  it("keeps the site's own tag without a campaign", () => {
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android downloads={null} sid="s1" />);
    expect(screen.getByRole("link", { name: /Get it on Google Play/ }).getAttribute("href")).toBe(googlePlayHref());
  });
});

describe("download_page_exit (legacy inventory 2.2)", () => {
  const setHidden = (hidden: boolean) => {
    Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
    document.dispatchEvent(new Event("visibilitychange"));
  };
  const exits = () => vi.mocked(track).mock.calls.filter(([event]) => event === "download_page_exit");
  afterEach(() => {
    Object.defineProperty(document, "hidden", { configurable: true, get: () => false });
    window.history.replaceState({}, "", "/");
  });

  it("reports leaving without a choice, with the time on the page and the page's utm_*, as a conversion", () => {
    window.history.replaceState({}, "", "/download?utm_source=Partner%20X&utm_medium=email");
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" userId="0xabc" />);
    setHidden(true);
    expect(exits()).toHaveLength(1);
    const [, props, ctx] = exits()[0]!;
    expect(Object.keys(props as object)).toEqual(["cta_clicked", "ms_on_page", "utm_source", "utm_medium"]);
    expect(props).toEqual({ cta_clicked: false, ms_on_page: expect.any(Number), utm_source: "partner_x", utm_medium: "email" });
    expect(ctx).toEqual({ sid: "s1", userId: "0xabc", legacy: true, conversion: true });
  });

  it("reports a download control clicked on this page, and fires again on every hide", async () => {
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" />);
    const link = screen.getByRole("link", { name: "Download on Epic Games" });
    link.addEventListener("click", (e) => e.preventDefault());
    await userEvent.setup().click(link);
    setHidden(true);
    setHidden(false);
    setHidden(true);
    expect(exits().map(([, props]) => (props as { cta_clicked: boolean }).cta_clicked)).toEqual([true, true]);
  });

  it("does not count a click made before this page mounted", () => {
    trackDownloadClick({ place: "Landing Hero", event: "Download", download_target: "desktop_installer" }, { sid: "s1" });
    render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" />);
    setHidden(true);
    expect(exits()[0]![1]).toMatchObject({ cta_clicked: false });
  });

  it("sends nothing when the page becomes visible, or after it unmounts", () => {
    const { unmount } = render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} sid="s1" />);
    setHidden(false);
    unmount();
    setHidden(true);
    expect(exits()).toHaveLength(0);
  });
});
