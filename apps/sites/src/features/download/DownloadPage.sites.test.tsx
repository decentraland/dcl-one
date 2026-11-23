// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";

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
  expect(screen.getByRole("link", { name: "Download for Windows" }).getAttribute("href")).toBe("/download_success?os=Windows&arch=amd64");
  expect(screen.getByRole("link", { name: "macOS" }).getAttribute("href")).toBe("/download_success?os=macOS&arch=arm64");
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
