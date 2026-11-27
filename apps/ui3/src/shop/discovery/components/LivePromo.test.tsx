// The home page's promo tile: a still picture, and over it the wearable-preview iframe's avatar while the tile is on
// screen, as decentraland.org's Shop draws it.

import { act, cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags } from "../lib/remoteFlags";

const PREVIEW = "https://wearable-preview.decentraland.org";
const EMOTE = "urn:decentraland:matic:collections-v2:0x0c956c74518ed34afb7b137d9ddfdaea7ca13751:0";
const HOODIE = "urn:decentraland:matic:collections-v2:0xe9f388ae27c726c4772c85a194e9791b1a0a913c:0";

const env = vi.hoisted(() => ({ device: "full" as "full" | "lite", linkUndecided: false, probes: 0 }));
vi.mock("../../detail/lib/pickRenderer", () => ({
  pickRenderer: (measuredMbps: number | null = null) =>
    env.linkUndecided && measuredMbps === null ? { renderer: "full", reason: "optimistic-default" } : { renderer: env.device, reason: "connection-ok" },
}));
// A probe that never answers: the tile can only be told the link is undecided.
vi.mock("@web-core/connection-speed", () => ({
  measureDownlinkMbps: () => {
    env.probes += 1;
    return new Promise(() => {});
  },
}));
vi.mock("../../../wearable-preview/WearablePreview", () => ({
  default: () => {
    throw new Error("the promo must not draw the three.js scene");
  },
}));
vi.mock("@dcl/one-ui/components/AnimatedBackground/AnimatedBackground", () => ({
  AnimatedBackground: (props: { patternTiling?: number }) => <div data-testid="backdrop" data-tiling={props.patternTiling} />,
}));

import { resetMeasuredLinkForTests } from "../../detail/lib/previewDevice";
import LivePromo from "./LivePromo";

type Observed = { cb: IntersectionObserverCallback; el?: Element; init?: IntersectionObserverInit };
let observers: Observed[] = [];

class FakeIO {
  private o: Observed;
  constructor(cb: IntersectionObserverCallback, init?: IntersectionObserverInit) {
    this.o = { cb, init };
    observers.push(this.o);
  }
  observe(el: Element) {
    this.o.el = el;
  }
  disconnect() {
    observers = observers.filter((o) => o !== this.o);
  }
  unobserve() {}
  takeRecords() {
    return [];
  }
}

function intersect(visible: boolean) {
  act(() => {
    for (const o of [...observers]) o.cb([{ isIntersecting: visible, target: o.el } as IntersectionObserverEntry], o as unknown as IntersectionObserver);
  });
}

const frame = () => screen.queryByTitle("3D preview") as HTMLIFrameElement | null;
const params = () => new URL(frame()!.src).searchParams;
const fallback = () => screen.getByAltText("art");

function fromPreview(data: unknown) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source: frame()!.contentWindow, origin: PREVIEW }));
  });
}

beforeEach(() => {
  observers = [];
  env.device = "full";
  env.linkUndecided = false;
  env.probes = 0;
  resetMeasuredLinkForTests();
  vi.stubGlobal("IntersectionObserver", FakeIO);
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  // The tile draws with Babylon as upstream's does, whatever this flag says.
  primeRemoteFlags({ "dapps-unity-wearable-preview": true });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  primeRemoteFlags(null);
});

function tile() {
  return render(
    <MemoryRouter>
      <LivePromo to="/shop/items" urns={[EMOTE, HOODIE]} title="Express\nwith style" cta="Explore" ariaLabel="promo" fallback="/art.png" fallbackAlt="art" testId="promo" />
    </MemoryRouter>,
  );
}

describe("LivePromo", () => {
  it("runs the link probe only once the tile first nears the screen, and mounts nothing while it runs", () => {
    env.linkUndecided = true;
    tile();
    expect(env.probes).toBe(0);
    intersect(true);
    expect(env.probes).toBe(1);
    expect(frame()).toBeNull();
    intersect(false);
    intersect(true);
    expect(env.probes).toBe(1);
  });

  it("mounts upstream's Babylon avatar in the iframe once the tile nears the screen, with nothing of three.js", () => {
    tile();
    expect(frame()).toBeNull();
    expect(observers[0]?.init?.rootMargin).toBe("200px");

    intersect(true);
    expect(new URL(frame()!.src).origin).toBe(PREVIEW);
    expect(params().getAll("urn")).toEqual([EMOTE, HOODIE]);
    expect(params().get("profile")).toBe("default");
    expect(params().get("type")).toBe("avatar");
    expect(params().get("emote")).toBe("fashion");
    expect(params().get("zoom")).toBe("100");
    expect(params().get("wheelZoom")).toBe("2.5");
    expect(params().get("disableBackground")).toBe("true");
    expect(params().has("unity")).toBe(false);
    expect(params().has("env")).toBe(false);
  });

  it("keeps the still picture until the avatar has drawn, then plays it muted and looping", () => {
    tile();
    intersect(true);
    const posted = vi.spyOn(frame()!.contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    expect(fallback().hasAttribute("data-hidden")).toBe(false);

    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);
    expect(screen.getByTestId("promo").hasAttribute("data-crest")).toBe(true);
    fromPreview({ type: "emote_event", payload: { type: "animation_end" } });
    const methods = posted.mock.calls.map(([message]) => (message as { payload?: { method?: string } }).payload?.method);
    expect(methods).toEqual(["disableSound", "play"]);
  });

  it("drops the iframe off screen, and back on screen keeps the picture until the new one draws", () => {
    tile();
    intersect(true);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);

    intersect(false);
    expect(frame()).toBeNull();
    expect(fallback().hasAttribute("data-hidden")).toBe(false);

    intersect(true);
    expect(frame()).not.toBeNull();
    expect(fallback().hasAttribute("data-hidden")).toBe(false);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);
  });

  it("keeps the picture for good once the avatar fails", () => {
    tile();
    intersect(true);
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();
    expect(fallback().hasAttribute("data-hidden")).toBe(false);

    intersect(false);
    intersect(true);
    expect(frame()).toBeNull();
  });

  it("keeps the picture with reduced motion, and on a device the Shop draws no full 3D scene on", () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    tile();
    intersect(true);
    expect(frame()).toBeNull();
    cleanup();

    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    env.device = "lite";
    tile();
    intersect(true);
    expect(frame()).toBeNull();
  });

  it("with the Bevy flag on draws with Bevy, and Bevy failing gives way to Babylon, covered until it draws", () => {
    primeRemoteFlags({ "dapps-shop-bevy-avatar-preview": true });
    tile();
    intersect(true);
    expect(new URL(frame()!.src).pathname).toBe("/avatar-preview/");
    expect(params().get("mode")).toBe("marketplace");
    expect(params().get("zoom")).toBe("100");
    const fromBevy = (data: unknown) =>
      act(() => {
        window.dispatchEvent(new MessageEvent("message", { data, source: frame()!.contentWindow, origin: window.location.origin }));
      });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);

    fromBevy({ type: "error", payload: { message: "lost the device" } });
    expect(new URL(frame()!.src).origin).toBe(PREVIEW);
    expect(params().has("mode")).toBe(false);
    expect(fallback().hasAttribute("data-hidden")).toBe(false);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);
  });

  it("under Bevy a frame that boots again is covered by the picture until its replacement draws", () => {
    primeRemoteFlags({ "dapps-shop-bevy-avatar-preview": true });
    tile();
    intersect(true);
    const fromBevy = (data: unknown) =>
      act(() => {
        window.dispatchEvent(new MessageEvent("message", { data, source: frame()!.contentWindow, origin: window.location.origin }));
      });
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);
    const first = frame()!.contentWindow;
    fromBevy({ type: "ready" });
    expect(frame()!.contentWindow).not.toBe(first);
    expect(fallback().hasAttribute("data-hidden")).toBe(false);
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);
  });

  it("goes by the last entry of a batch: in and out again is off screen", () => {
    tile();
    act(() => {
      for (const o of [...observers])
        o.cb(
          [{ isIntersecting: true, target: o.el } as IntersectionObserverEntry, { isIntersecting: false, target: o.el } as IntersectionObserverEntry],
          o as unknown as IntersectionObserver,
        );
    });
    expect(frame()).toBeNull();
  });

  it("waits for the flags before mounting, so it never loads one renderer and then the other", () => {
    primeRemoteFlags(null);
    vi.stubGlobal("fetch", () => new Promise(() => {}));
    tile();
    intersect(true);
    expect(frame()).toBeNull();
  });

  it("draws the fitting room's drifting backdrop under the avatar while on screen, at tiling 1", async () => {
    tile();
    expect(screen.queryByTestId("backdrop")).toBeNull();
    intersect(true);
    expect((await screen.findByTestId("backdrop")).getAttribute("data-tiling")).toBe("1");
    intersect(false);
    expect(screen.queryByTestId("backdrop")).toBeNull();
  });
});
