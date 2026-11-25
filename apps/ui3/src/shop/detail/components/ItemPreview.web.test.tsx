// The item preview on the web: the wearable-preview iframe decentraland.org's Shop embeds, mounted once with its final
// avatar and renderer, told later changes in place, and trusted only for messages its own frame sends.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { invalidateShopQueries } from "../../discovery/query";
import { primeRemoteFlags } from "../../discovery/lib/remoteFlags";
import ItemPreview from "./ItemPreview";

const PREVIEW = "https://wearable-preview.decentraland.org";
const ACCOUNT = "0x1111111111111111111111111111111111111111";
const MALE = "urn:decentraland:off-chain:base-avatars:BaseMale";
const FEMALE = "urn:decentraland:off-chain:base-avatars:BaseFemale";
const URN = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:0";

const env = vi.hoisted(() => ({
  account: null as string | null,
  device: "full" as "full" | "lite",
  profile: null as null | ((profile: unknown) => void),
}));

vi.mock("../../wallet/context", () => ({ useShopWallet: () => ({ account: env.account }) }));
vi.mock("../lib/pickRenderer", () => ({
  pickRenderer: () => ({ renderer: env.device, reason: env.device === "full" ? "connection-ok" : "mobile" }),
}));
vi.mock("../../../data/catalyst/profile", () => ({
  fetchProfile: () => new Promise((resolve) => (env.profile = resolve)),
}));
vi.mock("../../../wearable-preview/WearablePreview", () => ({
  default: () => {
    throw new Error("the web page must not draw the three.js scene");
  },
}));

const item = (over: Partial<CatalogItem> = {}) =>
  ({ id: "item-0", urn: URN, name: "Hoodie", category: "wearable", wearableCategory: "upper_body", thumbnail: "https://thumbs.example/hoodie.png", ...over }) as unknown as CatalogItem;

const frame = () => screen.queryByTitle("3D preview") as HTMLIFrameElement | null;
const params = () => new URL(frame()!.src).searchParams;
const thumb = () => screen.queryByRole("img", { name: "Hoodie" });

function fromPreview(data: unknown, source: Window | null = frame()!.contentWindow, origin = PREVIEW) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source, origin }));
  });
}

function flag(unity: boolean) {
  primeRemoteFlags({ "dapps-unity-wearable-preview": unity });
}

beforeEach(() => {
  env.account = null;
  env.device = "full";
  env.profile = null;
  invalidateShopQueries();
  flag(true);
});
afterEach(() => {
  cleanup();
  primeRemoteFlags(null);
});

describe("what the iframe mounts with", () => {
  test("a signed-out shopper sees the item worn by a default avatar, through Unity, under the thumbnail until it draws", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    expect(frame()!.src.startsWith(`${PREVIEW}/?`)).toBe(true);
    expect(params().getAll("urn")).toEqual([URN]);
    expect(params().get("profile")).toBe("default");
    expect(params().get("bodyShape")).toBe(MALE);
    expect(params().get("type")).toBe("avatar");
    expect(params().get("emote")).toBe("fashion");
    expect(params().get("disableBackground")).toBe("true");
    expect(params().get("unity")).toBe("true");
    expect(params().get("mode")).toBe("marketplace");
    expect(thumb()).not.toBeNull();

    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(thumb()).toBeNull();
    // Unity draws its own view and emote controls.
    expect(screen.queryByTestId("preview-toggle")).toBeNull();
  });

  test("with the flag off, or on a lighter device, it asks for Babylon", () => {
    flag(false);
    const view = render(<ItemPreview item={item()} renderer="full" />);
    expect(params().has("unity")).toBe(false);
    view.unmount();

    flag(true);
    env.device = "lite";
    render(<ItemPreview item={item()} />);
    expect(params().has("unity")).toBe(false);
  });

  test("a signed-in shopper waits for their profile, then sees the item on their own avatar", async () => {
    env.account = ACCOUNT;
    render(<ItemPreview item={item()} renderer="full" />);
    expect(frame()).toBeNull();
    await act(async () => {
      await vi.waitFor(() => expect(env.profile).not.toBeNull());
      env.profile!({ avatar: { bodyShape: FEMALE, wearables: [] } });
    });
    expect(params().get("profile")).toBe(ACCOUNT);
    expect(params().has("bodyShape")).toBe(false);
  });

  test("an avatar whose body cannot wear the item gives way to a default one, with a note", async () => {
    env.account = ACCOUNT;
    render(<ItemPreview item={item({ gender: "male" } as Partial<CatalogItem>)} renderer="full" />);
    await act(async () => {
      await vi.waitFor(() => expect(env.profile).not.toBeNull());
      env.profile!({ avatar: { bodyShape: FEMALE, wearables: [] } });
    });
    expect(params().get("profile")).toBe("default");
    expect(params().get("bodyShape")).toBe(MALE);
    expect(screen.getByTestId("shape-note")).toBeTruthy();
  });

  test("a testnet item is read from the testnet preview", () => {
    render(<ItemPreview item={item({ urn: "urn:decentraland:amoy:collections-v2:0xabc:0" })} renderer="full" />);
    expect(frame()!.src.startsWith("https://wearable-preview.decentraland.zone/?")).toBe(true);
    expect(params().get("env")).toBe("dev");
  });

  test("an emote plays on the avatar, with no view type and a closer wheel zoom", () => {
    flag(false);
    render(<ItemPreview item={item({ category: "emote" } as Partial<CatalogItem>)} renderer="full" />);
    expect(params().has("type")).toBe(false);
    expect(params().has("emote")).toBe(false);
    expect(params().get("wheelZoom")).toBe("1.5");
  });
});

describe("talking to the iframe", () => {
  test("over Babylon the page's view switch reloads the scene in place, as the item alone", () => {
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    const win = frame()!.contentWindow!;
    const posted = vi.spyOn(win, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });

    fireEvent.click(screen.getByRole("button", { name: "Item" }));
    expect(frame()!.contentWindow).toBe(win);
    const [message, origin] = posted.mock.calls.at(-1)!;
    expect(origin).toBe(PREVIEW);
    expect(message).toMatchObject({ type: "update", payload: { options: { type: "wearable", urns: [URN] } } });
    expect((message as { payload: { options: { profile?: string } } }).payload.options.profile).toBeUndefined();
    // The thumbnail covers the reload until the preview says it has drawn again.
    expect(thumb()).not.toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(thumb()).toBeNull();
  });

  test("over Babylon an emote's play button asks the preview's controller to play it again", () => {
    flag(false);
    render(<ItemPreview item={item({ category: "emote" } as Partial<CatalogItem>)} renderer="full" />);
    const posted = vi.spyOn(frame()!.contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    fireEvent.click(screen.getByRole("button", { name: "Play emote" }));
    expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "controller_request", payload: { namespace: "emote", method: "play" } });
  });

  test("Unity failing to load falls back to Babylon in a new iframe", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    expect(params().get("unity")).toBe("true");
    fromPreview({ type: "error", payload: { message: "unity failed" } });
    expect(frame()).not.toBeNull();
    expect(params().has("unity")).toBe(false);
  });

  test("Babylon failing leaves the thumbnail with the unavailable note", () => {
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();
    expect(thumb()).not.toBeNull();
    expect(screen.getByRole("status")).toBeTruthy();
  });

  test("messages from another window or origin are ignored", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "load", payload: { renderer: "unity" } }, window);
    fromPreview({ type: "load", payload: { renderer: "unity" } }, frame()!.contentWindow, "https://evil.example");
    expect(thumb()).not.toBeNull();
  });
});
