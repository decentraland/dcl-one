// @vitest-environment jsdom
import "../locale/test-setup";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const track = vi.fn();
const launch = vi.fn();
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));
vi.mock("./launch", async (actual) => ({
  ...(await actual<typeof import("./launch")>()),
  launchDesktopApp: (...args: unknown[]) => launch(...args),
}));
vi.mock("./mac-arch", () => ({ macArchHint: () => null }));

import { useDownloadPage } from "./useDownloadPage";

const ANON = "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f";
const SEARCH = `?utm_source=Partner&position=10%2C20&realm=fenrir&communityId=c-1&referrer=0x1111111111111111111111111111111111111111&anon_user_id=${ANON}&dclenv=zone`;
const session = { sid: "sid-1", story: "download" };

beforeEach(() => {
  track.mockReset();
  launch.mockReset();
  sessionStorage.clear();
});

function hide() {
  Object.defineProperty(document, "visibilityState", {
    value: "hidden",
    configurable: true,
  });
  document.dispatchEvent(new Event("visibilitychange"));
}

describe("useDownloadPage", () => {
  it("sends the Click and builds the /download_success URL with attribution and deep link", () => {
    const { result } = renderHook(() =>
      useDownloadPage(SEARCH, session, false),
    );

    const href = result.current.chooseInstaller({ os: "macos", arch: "arm64" });

    const url = new URL(href, "https://site.example");
    expect(url.pathname).toBe("/download_success");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      os: "macOS",
      place: "download-page",
      anon_user_id: ANON,
      arch: "arm64",
      referrer: "0x1111111111111111111111111111111111111111",
      position: "10,20",
      realm: "fenrir",
      communityId: "c-1",
      utm_source: "partner",
    });
    expect(track).toHaveBeenCalledWith(
      "Click",
      expect.objectContaining({
        place: "download-page",
        os: "macOS",
        download_target: "desktop_installer",
        utm_source: "partner",
      }),
      session,
    );
  });

  it("tracks a store exit without a click correlation", () => {
    const { result } = renderHook(() => useDownloadPage(SEARCH, session, true));

    act(() => result.current.leaveForStore("google_play"));

    expect(track.mock.calls[0]![1]).toMatchObject({
      os: "Android",
      download_target: "google_play",
    });
    expect(track.mock.calls[0]![1]).not.toHaveProperty("click_id");
  });

  it("launches the app with position, realm, community and the validated flags", async () => {
    launch.mockResolvedValue(true);
    const { result } = renderHook(() =>
      useDownloadPage(SEARCH, session, false),
    );

    await act(() => result.current.jumpIn());

    expect(launch).toHaveBeenCalledWith({
      position: "10,20",
      realm: "fenrir",
      communityId: "c-1",
      dclenv: "zone",
      sceneConsole: undefined,
      multiInstance: undefined,
      mobile: false,
    });
    expect(result.current.launcherMissing).toBe(false);
  });

  it("reports a missing launcher, and lets it be dismissed", async () => {
    launch.mockResolvedValue(false);
    const { result } = renderHook(() =>
      useDownloadPage(SEARCH, session, false),
    );

    await act(() => result.current.jumpIn());
    expect(result.current.launcherMissing).toBe(true);
    act(() => result.current.dismissLauncherMissing());
    expect(result.current.launcherMissing).toBe(false);
  });

  it("sends download_page_exit on every hide, with whether a CTA was clicked", () => {
    const { result } = renderHook(() =>
      useDownloadPage(SEARCH, session, false),
    );

    hide();
    result.current.chooseInstaller({ os: "windows", arch: "amd64" });
    hide();

    const exits = track.mock.calls
      .filter((call) => call[0] === "download_page_exit")
      .map((call) => call[1]);
    expect(exits.map((exit) => exit.cta_clicked)).toEqual([false, true]);
    expect(exits[0]).toMatchObject({
      utm_source: "partner",
      ms_on_page: expect.any(Number),
    });
  });

  it("removes the onboarding email and user from the address bar", () => {
    window.history.replaceState(
      null,
      "",
      "/download?email=a%40b.example&user=0xabc&utm_source=x",
    );
    renderHook(() =>
      useDownloadPage(
        "?email=a%40b.example&user=0xabc&utm_source=x",
        session,
        false,
      ),
    );

    expect(window.location.search).toBe("?utm_source=x");
  });
});
