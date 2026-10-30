
import { act, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let emitStatus: ((s: string) => void) | null = null;

vi.mock("../../detail/lib/pickRenderer", () => ({ pickRenderer: () => ({ renderer: "full" }) }));
vi.mock("../../../wearable-preview/WearablePreview", () => ({
  default: ({ onStatus }: { onStatus?: (s: string) => void }) => {
    emitStatus = onStatus ?? null;
    return <div data-testid="avatar" />;
  },
}));

import LivePromo from "./LivePromo";

type Observed = { cb: IntersectionObserverCallback; el?: Element };
let observers: Observed[] = [];

class FakeIO {
  private o: Observed;
  constructor(cb: IntersectionObserverCallback) {
    this.o = { cb };
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

beforeEach(() => {
  vi.useFakeTimers();
  observers = [];
  emitStatus = null;
  vi.stubGlobal("IntersectionObserver", FakeIO);
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function tile() {
  return render(
    <MemoryRouter>
      <LivePromo to="/shop/items" urns={["urn:x"]} title="Express\nwith style" cta="Explore" ariaLabel="promo" fallback="/art.png" fallbackAlt="art" testId="promo" />
    </MemoryRouter>,
  );
}

const fallback = () => screen.getByAltText("art");

describe("LivePromo", () => {
  it("keeps the static art until the avatar has settled on screen, and restores it off screen", async () => {
    tile();
    intersect(true);
    await act(async () => {
      await vi.dynamicImportSettled?.();
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(emitStatus).toBeTypeOf("function");
    act(() => emitStatus!("ready"));
    expect(fallback().hasAttribute("data-hidden")).toBe(false);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });
    expect(fallback().hasAttribute("data-hidden")).toBe(true);

    intersect(false);
    expect(fallback().hasAttribute("data-hidden")).toBe(false);
  });

  it("never hides the art when the avatar fails", async () => {
    tile();
    intersect(true);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    if (emitStatus) act(() => emitStatus!("error"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(fallback().hasAttribute("data-hidden")).toBe(false);
  });
});
