// The item preview on the web: the wearable-preview iframe decentraland.org's Shop embeds, mounted once with its final
// avatar and renderer, told later changes in place, and trusted only for messages its own frame sends.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { invalidateShopQueries, primeShopQuery } from "../../discovery/query";
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
  vi.unstubAllGlobals();
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

  test("a flags file that cannot be read leaves the preview on Babylon, not waiting", async () => {
    primeRemoteFlags(null);
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("flags down")));
    render(<ItemPreview item={item()} renderer="full" />);
    await vi.waitFor(() => expect(frame()).not.toBeNull());
    expect(params().has("unity")).toBe(false);
  });

  test("a lighter device mounts Babylon without waiting for the flags file", () => {
    primeRemoteFlags(null);
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    env.device = "lite";
    render(<ItemPreview item={item()} />);
    expect(frame()).not.toBeNull();
    expect(params().has("unity")).toBe(false);
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
  test("a frame that boots again after a view switch is told the current view again, not left on its mount URL's", () => {
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    const posted = vi.spyOn(frame()!.contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    fireEvent.click(screen.getByRole("button", { name: "Item" }));
    const updates = () => posted.mock.calls.filter(([message]) => (message as { type?: string }).type === "update");
    expect(updates()).toHaveLength(1);

    // The frame reloads itself: it boots from its mount URL, the avatar view.
    fromPreview({ type: "ready" });
    expect(updates()).toHaveLength(2);
    expect(updates().at(-1)![0]).toMatchObject({ type: "update", payload: { options: { type: "wearable" } } });
  });

  test("a frame that boots again with nothing changed since its mount is not sent an update", () => {
    // Babylon: a Unity frame that boots again is replaced instead (PreviewFrame).
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    const posted = vi.spyOn(frame()!.contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "ready" });
    expect(posted).not.toHaveBeenCalled();
  });

  test("the item alone failing where the avatar had drawn goes back to the avatar, in a fresh frame with its controls", () => {
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    const first = frame();
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    fireEvent.click(screen.getByRole("button", { name: "Item" }));
    fromPreview({ type: "error", payload: { message: "no item alone" } });

    expect(frame()).not.toBe(first);
    expect(params().get("type")).toBe("avatar");
    expect(thumb()).not.toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(screen.getByRole("button", { name: "On avatar" }).getAttribute("aria-pressed")).toBe("true");
  });

  test("signing in with the preview open mounts a new frame for the shopper, covered until it draws", async () => {
    flag(false);
    const view = render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(screen.queryByTestId("preview-toggle")).not.toBeNull();

    env.account = ACCOUNT;
    view.rerender(<ItemPreview item={item()} renderer="full" />);
    await act(async () => {
      await vi.waitFor(() => expect(env.profile).not.toBeNull());
      env.profile!({ avatar: { bodyShape: MALE, wearables: [] } });
    });
    expect(params().get("profile")).toBe(ACCOUNT);
    expect(thumb()).not.toBeNull();
    expect(screen.queryByTestId("preview-toggle")).toBeNull();
  });

  test("signing in and out before the profile is read mounts a new guest frame, covered until it draws", () => {
    flag(false);
    const view = render(<ItemPreview item={item()} renderer="full" />);
    const first = frame();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(thumb()).toBeNull();

    env.account = ACCOUNT;
    view.rerender(<ItemPreview item={item()} renderer="full" />);
    expect(frame()).toBeNull();
    env.account = null;
    view.rerender(<ItemPreview item={item()} renderer="full" />);

    expect(frame()).not.toBeNull();
    expect(frame()).not.toBe(first);
    expect(thumb()).not.toBeNull();
    expect(screen.queryByTestId("preview-toggle")).toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(thumb()).toBeNull();
  });

  test("with the profile already read, signing in and out before the account's frame draws covers the new guest frame", () => {
    flag(false);
    primeShopQuery(["wearer-profile", ACCOUNT], { avatar: { bodyShape: MALE, wearables: [] } });
    const view = render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(thumb()).toBeNull();

    env.account = ACCOUNT;
    view.rerender(<ItemPreview item={item()} renderer="full" />);
    expect(params().get("profile")).toBe(ACCOUNT);
    env.account = null;
    view.rerender(<ItemPreview item={item()} renderer="full" />);

    expect(params().get("profile")).toBe("default");
    expect(thumb()).not.toBeNull();
    expect(screen.queryByTestId("preview-toggle")).toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(thumb()).toBeNull();
  });

  test("a Unity request that drew with Babylon goes back to the avatar when the item alone fails, not to the item again", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    expect(params().get("unity")).toBe("true");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    fireEvent.click(screen.getByRole("button", { name: "Item" }));
    fromPreview({ type: "error", payload: { message: "no item alone" } });

    expect(params().get("type")).toBe("avatar");
    expect(thumb()).not.toBeNull();
  });

  test("a new item starts over: avatar view, and Unity tried again after the last item fell back", () => {
    const view = render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "error", payload: { message: "unity failed" } });
    expect(params().has("unity")).toBe(false);

    const next = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1";
    view.rerender(<ItemPreview item={item({ id: "item-1", urn: next })} renderer="full" />);
    expect(params().get("unity")).toBe("true");
    expect(params().get("type")).toBe("avatar");
  });

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

  test("Unity that neither draws nor fails within 45 seconds gives way to Babylon", () => {
    vi.useFakeTimers();
    try {
      render(<ItemPreview item={item()} renderer="full" />);
      expect(params().get("unity")).toBe("true");
      fromPreview({ type: "ready" });
      act(() => vi.advanceTimersByTime(45_000));
      expect(params().has("unity")).toBe(false);
      expect(thumb()).not.toBeNull();

      // Babylon is given no timeout: it says when it fails.
      act(() => vi.advanceTimersByTime(10 * 60_000));
      expect(frame()).not.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  test("Babylon failing leaves the thumbnail with the unavailable note", () => {
    flag(false);
    render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();
    expect(thumb()).not.toBeNull();
    expect(screen.getByRole("status")).toBeTruthy();
  });

  test("under Unity a drawn frame that boots again shows the thumbnail until its replacement draws", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(thumb()).toBeNull();
    const first = frame()!.contentWindow;

    fromPreview({ type: "ready" });
    expect(frame()!.contentWindow).not.toBe(first);
    expect(thumb()).not.toBeNull();
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(thumb()).toBeNull();
  });

  test("messages from another window or origin are ignored", () => {
    render(<ItemPreview item={item()} renderer="full" />);
    fromPreview({ type: "load", payload: { renderer: "unity" } }, window);
    fromPreview({ type: "load", payload: { renderer: "unity" } }, frame()!.contentWindow, "https://evil.example");
    expect(thumb()).not.toBeNull();
  });
});

describe("the Bevy avatar preview", () => {
  const bevy = (unity: boolean) => primeRemoteFlags({ "dapps-unity-wearable-preview": unity, "dapps-shop-bevy-avatar-preview": true });
  const fromBevy = (data: unknown) => fromPreview(data, frame()!.contentWindow, window.location.origin);

  test("with its flag on, the item page draws with Bevy in marketplace mode, with no controls of the page's own", () => {
    bevy(true);
    render(<ItemPreview item={item()} renderer="full" />);
    expect(frame()!.src.startsWith(`${window.location.origin}/avatar-preview/?`)).toBe(true);
    expect(params().get("mode")).toBe("marketplace");
    expect(params().get("profile")).toBe("default");
    expect(params().has("unity")).toBe(false);
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(thumb()).toBeNull();
    expect(screen.queryByTestId("preview-toggle")).toBeNull();
  });

  test("an emote leaves Bevy its own controls, its playback among them: no framing that hides them, and no second playbar", () => {
    bevy(false);
    render(<ItemPreview item={item({ category: "emote" } as Partial<CatalogItem>)} renderer="full" />);
    for (const hides of ["wheelZoom", "zoom", "hideControls", "camera", "showThumbnailBoundaries"]) expect(params().has(hides)).toBe(false);
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(screen.queryByRole("button", { name: "Play emote" })).toBeNull();
  });

  test("Bevy failing gives way to Unity, then Babylon; a new item tries Bevy again", () => {
    bevy(true);
    const view = render(<ItemPreview item={item()} renderer="full" />);
    fromBevy({ type: "error", payload: { message: "renderer failed to start" } });
    expect(frame()!.src.startsWith(`${PREVIEW}/?`)).toBe(true);
    expect(params().get("unity")).toBe("true");
    fromPreview({ type: "error", payload: { message: "unity failed" } });
    expect(params().has("unity")).toBe(false);
    expect(frame()!.src.startsWith(`${PREVIEW}/?`)).toBe(true);

    view.rerender(<ItemPreview item={item({ urn: "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1" } as Partial<CatalogItem>)} renderer="full" />);
    expect(frame()!.src.startsWith(`${window.location.origin}/avatar-preview/?`)).toBe(true);
  });
});
