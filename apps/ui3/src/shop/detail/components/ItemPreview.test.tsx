// The item preview inside the Explorer, while the worn outfit loads: after the wearer's profile the scene mounts under
// the thumbnail and loads nothing until the outfit comes; the thumbnail stays until the first drawn frame, and failures
// show over it. The web preview, the wearable-preview iframe, is ItemPreview.web.test.tsx.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { AvatarStatus } from "../../../wearable-preview/avatar";
import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { invalidateShopQueries } from "../../discovery/query";
import ItemPreview from "./ItemPreview";

type PreviewProps = { base?: string; outfit?: { wearables?: string[] }; outfitPending?: boolean; onStatus?: (s: AvatarStatus) => void };
const previews = vi.hoisted(() => ({ props: [] as PreviewProps[], mounts: 0 }));
const reads = vi.hoisted(() => ({ profile: null as null | ((profile: unknown) => void), rules: null as null | ((rules: unknown[]) => void), rulesBase: undefined as string | undefined }));

vi.mock("../../../wearable-preview/WearablePreview", async () => {
  const { useEffect } = await import("react");
  return {
    default: (props: PreviewProps) => {
      previews.props.push(props);
      useEffect(() => {
        previews.mounts++;
      }, []);
      return <div data-testid="scene" />;
    },
  };
});
vi.mock("../../../data/catalyst/profile", () => ({
  fetchProfile: () => new Promise((resolve) => (reads.profile = resolve)),
}));
vi.mock("../lib/wearable-rules", async (importOriginal) => ({
  // The real choice of Catalyst, so the test sees what the two reads would actually use.
  ...(await importOriginal<typeof import("../lib/wearable-rules")>()),
  fetchWearableRules: (_urns: string[], _also: string[], base?: string) => {
    reads.rulesBase = base;
    return new Promise((resolve) => (reads.rules = resolve));
  },
  keepEquipped: () => [],
}));

const URN = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:0";
const ITEM = { id: "0x0768847951fddd4e73e670800da26d953a77059b-0", urn: URN, name: "Hoodie", category: "wearable", wearableCategory: "upper_body", thumbnail: "https://thumbs.example/hoodie.png" } as unknown as CatalogItem;
const BODY = "urn:decentraland:off-chain:base-avatars:BaseMale";

const last = () => previews.props.at(-1)!;
const thumb = () => screen.queryByRole("img", { name: "Hoodie" });

beforeEach(() => {
  // The Explorer announces its preview host; the page then draws its own scene.
  window.dclAvatarPreviewExpected = true;
  previews.props.length = 0;
  previews.mounts = 0;
  reads.profile = null;
  reads.rules = null;
  reads.rulesBase = undefined;
  invalidateShopQueries();
});
afterEach(() => {
  cleanup();
  delete window.dclAvatarPreviewExpected;
  delete window.dclAvatarPreview;
});

test("an Explorer that installs its preview host without announcing it still gets the Explorer's scene", async () => {
  delete window.dclAvatarPreviewExpected;
  window.dclAvatarPreview = { create: vi.fn() } as unknown as NonNullable<typeof window.dclAvatarPreview>;
  render(<ItemPreview item={ITEM} renderer="full" />);
  expect(screen.queryByTitle("3D preview")).toBeNull();
  await act(async () => {
    await vi.waitFor(() => expect(reads.profile).not.toBeNull());
    reads.profile!({ avatar: { bodyShape: BODY, wearables: [] } });
  });
  await vi.waitFor(() => expect(screen.getByTestId("scene")).toBeTruthy());
});

test("the scene mounts after the profile, during the outfit's entity read, with its loads held under the thumbnail, and gets the outfit once it is read", async () => {
  render(<ItemPreview item={ITEM} renderer="full" />);
  expect(screen.queryByTestId("scene")).toBeNull();
  expect(thumb()).not.toBeNull();
  expect(screen.queryByText("Loading preview")).toBeNull();

  await act(async () => {
    await vi.waitFor(() => expect(reads.profile).not.toBeNull());
    reads.profile!({ avatar: { bodyShape: BODY, wearables: [] } });
  });
  await vi.waitFor(() => expect(screen.getByTestId("scene")).toBeTruthy());
  expect(last().outfitPending).toBe(true);
  expect(last().outfit).toBeUndefined();
  expect(thumb()?.className).toContain("shop-preview__thumb--cover");
  expect(screen.queryByText("Loading preview")).toBeNull();

  await act(async () => {
    await vi.waitFor(() => expect(reads.rules).not.toBeNull());
    reads.rules!([]);
  });
  await vi.waitFor(() => expect(last().outfitPending).toBe(false));
  expect(last().outfit?.wearables).toEqual([URN]);
  expect(previews.mounts).toBe(1);
  expect(thumb()).not.toBeNull();

  act(() => last().onStatus!("ready"));
  expect(thumb()).toBeNull();
});

test("a failed or empty scene leaves the thumbnail in place with the note over it", async () => {
  for (const failure of ["error", "empty"] as const) {
    const view = render(<ItemPreview item={ITEM} renderer="full" />);
    await act(async () => {
      await vi.waitFor(() => expect(reads.profile).not.toBeNull());
      reads.profile!(null);
    });
    await vi.waitFor(() => expect(screen.getByTestId("scene")).toBeTruthy());
    act(() => last().onStatus!(failure));
    expect(screen.queryByTestId("scene")).toBeNull();
    expect(thumb()?.className).toBe("shop-preview__thumb");
    expect(screen.getByText("Preview unavailable for this item")).toBeTruthy();
    view.unmount();
    invalidateShopQueries();
    reads.profile = null;
  }
});

describe("the Catalyst the outfit's rules read and the scene use", () => {
  const peered = window as typeof window & { __SHOP_PEERS__?: Record<string, string> };
  afterEach(() => {
    delete peered.__SHOP_PEERS__;
  });
  async function readTheOutfit(wearables: string[]) {
    render(<ItemPreview item={ITEM} renderer="full" />);
    await act(async () => {
      await vi.waitFor(() => expect(reads.profile).not.toBeNull());
      reads.profile!({ avatar: { bodyShape: BODY, wearables } });
    });
    await vi.waitFor(() => expect(reads.rules).not.toBeNull());
  }

  test("one peer serves the whole outfit: both use it, so the scene reuses the entities the rules read cached", async () => {
    await readTheOutfit(["urn:decentraland:matic:collections-v2:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"]);
    expect(reads.rulesBase).toEqual(expect.any(String));
    expect(last().base).toBe(reads.rulesBase);
  });

  test("an outfit whose networks have different peers: each read keeps its own, the scene its default", async () => {
    peered.__SHOP_PEERS__ = { amoy: "https://peer.amoy.test" };
    await readTheOutfit(["urn:decentraland:amoy:collections-v2:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"]);
    expect(reads.rulesBase).toBeUndefined();
    expect(last().base).toBeUndefined();
  });
});
