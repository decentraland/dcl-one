// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";

import ReelsList from "./reels.list.$address";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

const readiness = vi.hoisted(() => ({ ready: true }));
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: () => () => {},
}));

const OWNER = "0x1111111111111111111111111111111111111111";
const photo: ProfilePhoto = {
  id: "9B1DEB4D-3B7D-4BAD-9BDD-2B0D7B3DCB6D",
  url: "https://camera-reel.example/9b1deb4d.jpg",
  thumbnailUrl: "https://camera-reel.example/9b1deb4d-thumb.jpg",
  isPublic: true,
  dateTime: "1700000000",
  realm: "main",
  scene: { name: "Genesis Plaza", x: "10", y: "-5" },
  author: { name: "Alice", address: OWNER },
  people: [],
};

function renderGallery() {
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        Component: Outlet,
        children: [
          { id: "gallery", path: "reels/list/:address", Component: ReelsList as never },
          { path: "*", Component: () => null },
        ],
      },
    ],
    {
      initialEntries: [`/reels/list/${OWNER}`],
      hydrationData: {
        loaderData: { root: { sid: "sid-root", analyticsUserId: "" }, gallery: { photos: [photo], total: 1 } },
      },
    },
  );
  render(<RouterProvider router={router} />);
}

beforeEach(() => {
  readiness.ready = true;
  renderGallery();
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("a Reels gallery, legacy events (decentraland/sites pages/reels/ReelsListPage.tsx:44)", () => {
  it("should send Reels Click Thumbnail with the photo's id when a photo is opened", async () => {
    await userEvent.click(screen.getByRole("link", { name: "Photo at Genesis Plaza, November 14, 2023" }));
    expect(vi.mocked(track).mock.calls).toEqual([
      ["Reels Click Thumbnail", { imageId: photo.id }, { sid: "sid-root", legacy: true }],
    ]);
  });

  describe("when the document's analytics is not ready", () => {
    beforeEach(() => {
      readiness.ready = false;
    });

    it("should send nothing", async () => {
      await userEvent.click(screen.getByRole("link", { name: "Photo at Genesis Plaza, November 14, 2023" }));
      expect(vi.mocked(track)).not.toHaveBeenCalled();
    });
  });
});
