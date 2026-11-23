// @vitest-environment jsdom
import { StrictMode, type ReactNode } from "react";
import { cleanup, render } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import CreatorHubDownloadSuccessRoute from "./download_.creator-hub-success";

// The Creator Hub's download_success on its steps page (legacy CreatorHubDownloadSuccess, inventory 2.2).

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshReg$: () => {}, $RefreshSig$: () => (t: unknown) => t }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn(), legacyAnalyticsReady: () => true, whenLegacyAnalyticsReady: (fire: () => void) => fire() }));
vi.mock("@/core/lib/telemetry/page-view", () => ({ useAnalyticsUserId: () => "" }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

const NOW = 1_800_000_000_000;
const ANON = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
const DMG = "https://github.com/decentraland/creator-hub/releases/download/1.2.3/hub-mac-arm64.dmg";

type Data = { sid: string; os: string; retryUrl: string; arch: "amd64" | "arm64"; exact: string | null; anonUserId: string | null };
const DEFAULTS: Data = { sid: "s1", os: "macos", retryUrl: DMG, arch: "arm64", exact: DMG, anonUserId: null };

function show(data: Partial<Data> = {}) {
  const loaderData = { ...DEFAULTS, ...data };
  const router = createMemoryRouter(
    [{ id: "success", path: "*", Component: CreatorHubDownloadSuccessRoute as never }],
    { initialEntries: ["/download/creator-hub-success"], hydrationData: { loaderData: { success: loaderData } } },
  );
  return render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
}
const successes = () => vi.mocked(track).mock.calls.filter(([event]) => event === "download_success");

beforeEach(() => {
  vi.spyOn(Date, "now").mockReturnValue(NOW);
  sessionStorage.clear();
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.mocked(track).mockReset();
});

describe("arriving on the Creator Hub steps page", () => {
  it("sends download_success once, with the build's link and file name, no delivery and no duration", () => {
    show({ anonUserId: ANON });
    expect(successes()).toHaveLength(1);
    const [, props, ctx] = successes()[0]!;
    expect(Object.keys(props as object)).toEqual([
      "download_target",
      "href",
      "os",
      "arch",
      "auth_state",
      "revisit",
      "place",
      "anon_user_id",
      "filename",
      "started_at",
      "succeeded_at",
      "duration_ms",
      "track_called_at",
      "track_delivered_at",
      "track_deferred",
    ]);
    expect(props).toEqual({
      download_target: "creator_hub",
      href: DMG,
      os: "macOS",
      arch: "arm64",
      auth_state: "anonymous",
      revisit: 0,
      place: "creator-hub-success-page",
      anon_user_id: ANON,
      filename: "hub-mac-arm64.dmg",
      started_at: NOW,
      succeeded_at: NOW,
      duration_ms: 0,
      track_called_at: NOW,
      track_delivered_at: NOW,
      track_deferred: true,
    });
    expect(ctx).toEqual({ sid: "s1", legacy: true, conversion: true });
    expect(vi.mocked(track).mock.calls.map(([event]) => event)).toContain("lp_creatorhub_download_success");
  });

  it("counts a revisit of the same build in this tab", () => {
    show();
    cleanup();
    show();
    expect(successes().map(([, props]) => (props as { revisit: number }).revisit)).toEqual([0, 1]);
    expect(sessionStorage.getItem("creatorHubDownloadSuccess:visits:macOS:arm64")).toBe("2");
  });

  it("sends nothing when the release holds no file for that build", () => {
    show({ os: "windows", arch: "arm64", exact: null, retryUrl: "" });
    expect(successes()).toHaveLength(0);
  });
});

