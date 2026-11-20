// The item preview while the worn outfit loads: after the wearer's profile the scene mounts under the thumbnail and
// loads nothing until the outfit comes; the thumbnail stays until the first drawn frame, and failures show over it.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

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
vi.mock("../lib/wearable-rules", () => ({
  entityBase: () => "https://peer.test",
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
  previews.props.length = 0;
  previews.mounts = 0;
  reads.profile = null;
  reads.rules = null;
  reads.rulesBase = undefined;
  invalidateShopQueries();
});
afterEach(cleanup);

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

test("the outfit's rules read and the scene use one Catalyst, so the scene reuses the entities that read cached", async () => {
  render(<ItemPreview item={ITEM} renderer="full" />);
  await act(async () => {
    await vi.waitFor(() => expect(reads.profile).not.toBeNull());
    reads.profile!({ avatar: { bodyShape: BODY, wearables: [] } });
  });
  await vi.waitFor(() => expect(reads.rules).not.toBeNull());
  expect(reads.rulesBase).toBe("https://peer.test");
  expect(last().base).toBe("https://peer.test");
});
