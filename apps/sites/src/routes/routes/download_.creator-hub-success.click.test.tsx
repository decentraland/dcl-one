// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";

import CreatorHubDownloadSuccessRoute from "./download_.creator-hub-success";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
// A document whose analytics is ready: its page view has been sent.
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));

afterEach(() => {
  cleanup();
  vi.mocked(track).mockReset();
});

async function renderRoute(loaderData: { sid: string; os: string; retryUrl: string }) {
  const router = createMemoryRouter(
    [{ id: "success", path: "/download/creator-hub-success", Component: CreatorHubDownloadSuccessRoute as never }],
    { initialEntries: ["/download/creator-hub-success"], hydrationData: { loaderData: { success: loaderData } } },
  );
  render(<RouterProvider router={router} />);
  const link = screen.getByRole("link", { name: "Download Decentraland Creator Hub" });
  link.addEventListener("click", (e) => e.preventDefault());
  await userEvent.setup().click(link);
  return vi.mocked(track).mock.calls.filter(([event]) => event === "Click");
}

it("row 27: the footer retry sends Creator Hub Success Footer / creator_hub with the page's system", async () => {
  const clicks = await renderRoute({ sid: "s1", os: "windows", retryUrl: "https://example.com/creator-hub.exe" });
  expect(clicks).toHaveLength(1);
  const [, props, ctx] = clicks[0]!;
  expect(props).toMatchObject({
    place: "Creator Hub Success Footer",
    event: "Download",
    download_target: "creator_hub",
    os: "Windows",
  });
  expect(props).not.toHaveProperty("click_id");
  expect(ctx).toEqual({ sid: "s1", legacy: true });
});

it("row 27: names macOS for a Mac", async () => {
  const clicks = await renderRoute({ sid: "s1", os: "macos", retryUrl: "https://example.com/creator-hub.dmg" });
  expect(clicks[0]![1]).toMatchObject({ os: "macOS" });
});

it("sends no Click for the releases-page fallback, as the legacy footer did without an installer", async () => {
  expect(await renderRoute({ sid: "s1", os: "macos", retryUrl: "" })).toHaveLength(0);
});

it("keeps lp_creatorhub_download_success unchanged", async () => {
  await renderRoute({ sid: "s1", os: "macos", retryUrl: "https://example.com/creator-hub.dmg" });
  expect(track).toHaveBeenCalledWith(
    "lp_creatorhub_download_success",
    { os: "macos" },
    { sid: "s1", story: "landings/creator-hub-download" },
  );
});
