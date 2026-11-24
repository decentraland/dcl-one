// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { wearableRef } from "@/data/lib/reels/queries";

import { ReelScreen } from "./ReelScreen";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

/** The document's readiness (its first page view), driven by the test. */
const readiness = vi.hoisted(() => ({ ready: true, pending: [] as Array<() => void> }));

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: (fire: () => void) => {
    if (readiness.ready) {
      fire();
      return () => {};
    }
    readiness.pending.push(fire);
    return () => {
      readiness.pending = readiness.pending.filter((queued) => queued !== fire);
    };
  },
}));

const AUTHOR = "0xAbCdEf0123456789aBcDeF0123456789AbCdEf01";
const PERSON = "0x1111111111111111111111111111111111111111";
const URN = "urn:decentraland:matic:collections-v2:0x2222222222222222222222222222222222222222:0:105312291668557186697918027683670432318895095400549111254310977538";
const ITEM = wearableRef(URN)!;

const wearables = vi.hoisted(() => ({ data: {} as Record<string, unknown> }));
vi.mock("@/features/profile/hooks", () => ({ usePlaceOwners: () => ({ data: {} }) }));
vi.mock("@/features/reels/hooks", () => ({
  useReelPlace: () => ({ data: "/places/genesis-plaza" }),
  useReelWearables: () => ({ data: wearables.data, isLoading: false }),
}));

const photo: ProfilePhoto = {
  id: "9B1DEB4D-3B7D-4BAD-9BDD-2B0D7B3DCB6D",
  url: "https://camera-reel.example/9b1deb4d.jpg",
  thumbnailUrl: "https://camera-reel.example/9b1deb4d-thumb.jpg",
  isPublic: true,
  dateTime: "1700000000",
  realm: "main",
  scene: { name: "Genesis Plaza", x: "10", y: "-5" },
  author: { name: "Alice", address: AUTHOR.toLowerCase(), sourceAddress: AUTHOR },
  people: [{ name: "Bob", address: PERSON, isGuest: false, wearables: [URN] }],
};

function renderReel() {
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        Component: Outlet,
        children: [
          { path: "reels/:id", Component: () => <ReelScreen photo={photo} origin="https://decentraland.example" /> },
          // Following a link leaves the photo: whatever it opens is not under test.
          { path: "*", Component: () => null },
        ],
      },
    ],
    {
      initialEntries: ["/reels/9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d"],
      hydrationData: { loaderData: { root: { sid: "sid-root", analyticsUserId: "0xabc" } } },
    },
  );
  render(<RouterProvider router={router} />);
}

const CTX = { sid: "sid-root", userId: "0xabc", legacy: true };

/** The events sent, each with its context checked. */
function sent() {
  return vi.mocked(track).mock.calls.map(([event, props, ctx]) => {
    expect(ctx).toEqual(CTX);
    return [event, props];
  });
}

/** jsdom cannot navigate; a real browser would leave or open a tab after the handlers ran. */
function blockNavigation(event: MouseEvent) {
  event.preventDefault();
}

let user: ReturnType<typeof userEvent.setup>;

beforeEach(() => {
  readiness.ready = true;
  readiness.pending = [];
  wearables.data = { [ITEM.key]: { id: ITEM.key, label: "Cool Hat", href: "/shop/items/cool-hat" } };
  document.addEventListener("click", blockNavigation);
  user = userEvent.setup();
  renderReel();
});

afterEach(() => {
  cleanup();
  document.removeEventListener("click", blockNavigation);
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("a Reels photo page, legacy events (decentraland/sites components/Reels, rows 2.3 and Click row 75)", () => {
  it("should send Reels Share with the image id when sharing to X", async () => {
    await user.click(screen.getByRole("link", { name: "Share to X" }));
    expect(sent()).toEqual([["Reels Share", { imageId: photo.id }]]);
  });

  it("should send Reels Copy Link once the link is on the clipboard", async () => {
    await user.click(screen.getByRole("button", { name: "Copy link" }));
    await screen.findByText("Link copied!");
    expect(sent()).toEqual([["Reels Copy Link", { imageId: photo.id }]]);
  });

  it("should send no Reels Copy Link when the copy fails", async () => {
    vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(new Error("denied"));
    await user.click(screen.getByRole("button", { name: "Copy link" }));
    await screen.findByText("Could not copy automatically. Copy this link instead.");
    expect(sent()).toEqual([]);
  });

  it("should send Reels Download at the press, even when the image cannot be fetched", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Promise.reject(new Error("offline"))));
    await user.click(screen.getByRole("button", { name: "Download photo" }));
    await screen.findByText("Could not download this photo. Try again.");
    expect(sent()).toEqual([["Reels Download", { imageId: photo.id }]]);
  });

  it("should send Reels Hide Metadata, then Reels Show Metadata, as the details are toggled", async () => {
    const info = screen.getByRole("button", { name: "Information" });
    await user.click(info);
    await user.click(info);
    expect(sent()).toEqual([
      ["Reels Hide Metadata", { imageId: photo.id }],
      ["Reels Show Metadata", { imageId: photo.id }],
    ]);
  });

  it("should send Reels Click Profile with the photographer's address as the service spelled it", async () => {
    await user.click(screen.getByRole("link", { name: "Alice" }));
    expect(sent()).toEqual([["Reels Click Profile", { userAddress: AUTHOR }]]);
  });

  it("should send Reels Click Place with the raw coordinates", async () => {
    await user.click(screen.getByRole("link", { name: "Genesis Plaza 10,-5" }));
    expect(sent()).toEqual([["Reels Click Place", { x: "10", y: "-5" }]]);
  });

  it("should send Reels Jump In with the raw coordinates", async () => {
    await user.click(screen.getByRole("link", { name: "Jump in to Genesis Plaza 10,-5" }));
    expect(sent()).toEqual([["Reels Jump In", { x: "10", y: "-5" }]]);
  });

  it("should send Reels Click Profile with the legacy profile URL for a person in the photo", async () => {
    await user.click(screen.getByRole("link", { name: "Bob" }));
    expect(sent()).toEqual([
      ["Reels Click Profile", { userAddress: PERSON, legacyProfileUrl: `https://profile.decentraland.org/accounts/${PERSON}` }],
    ]);
  });

  it("should show the first person's wearables open, as decentraland.org did", () => {
    expect((screen.getByText("Collectible wearables").parentElement as HTMLDetailsElement).open).toBe(true);
  });

  it("should send Reels Hide Wearables on the first toggle, then Reels Show Wearables", async () => {
    const summary = screen.getByText("Collectible wearables");
    await user.click(summary);
    // jsdom does not run a summary's activation: close the list as the browser would have.
    (summary.parentElement as HTMLDetailsElement).open = false;
    await user.click(summary);
    expect(sent()).toEqual([
      ["Reels Hide Wearables", { userAddress: PERSON }],
      ["Reels Show Wearables", { userAddress: PERSON }],
    ]);
  });

  it("should send Reels Click Wearable with the URN the wearable was worn as", async () => {
    await user.click(screen.getByRole("link", { name: "Cool Hat" }));
    expect(sent()).toEqual([["Reels Click Wearable", { wearableUrn: URN }]]);
  });

  it("should send the logo's Click with only its event and the audit fields", async () => {
    await user.click(screen.getByRole("link", { name: "Decentraland home" }));
    const [[event, props]] = sent() as [[string, Record<string, unknown>]];
    const { track_called_at, track_delivered_at, ...rest } = props;
    expect(event).toBe("Click");
    expect(rest).toEqual({ event: "Reels Click Decentraland Logo", track_deferred: false });
    expect(typeof track_called_at).toBe("number");
    expect(typeof track_delivered_at).toBe("number");
  });

  describe("when the document's analytics is not ready (a bot, an exempt session, no write key)", () => {
    beforeEach(() => {
      readiness.ready = false;
    });

    it("should send none of the Reels events, then or later", async () => {
      await user.click(screen.getByText("Collectible wearables"));
      await user.click(screen.getByRole("link", { name: "Share to X" }));
      await user.click(screen.getByRole("button", { name: "Information" }));
      expect(readiness.pending).toEqual([]);
      expect(sent()).toEqual([]);
    });

    it("should hold the logo's Click and drop it when the page goes away", async () => {
      await user.click(screen.getByRole("link", { name: "Decentraland home" }));
      expect(sent()).toEqual([]);
      cleanup();
      expect(readiness.pending).toEqual([]);
    });
  });
});
