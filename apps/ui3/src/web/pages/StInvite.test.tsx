import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import StInvite, { referralUrl } from "./StInvite";

describe("referralUrl", () => {
  it("puts the referrer on the sign-in and on its redirect to the download", () => {
    const url = new URL(referralUrl("0xabc"), "https://example.org");
    expect(url.pathname).toBe("/auth/login");
    expect(url.searchParams.get("referrer")).toBe("0xabc");
    expect(url.searchParams.get("redirectTo")).toBe("/download?referrer=0xabc");
  });

  it("sends a visit without a referrer to the download after the sign-in", () => {
    const url = new URL(referralUrl(""), "https://example.org");
    expect(url.searchParams.has("referrer")).toBe(false);
    expect(url.searchParams.get("redirectTo")).toBe("/download");
  });
});

describe("StInvite", () => {
  it("links both heroes' CTAs to the referral sign-in", () => {
    render(<StInvite referrer={{ name: "bevynaut2", ethAddress: "0xABC" }} />);
    const expected = referralUrl("0xabc");
    expect(screen.getByRole("link", { name: "JOIN NOW" })).toHaveAttribute("href", expected);
    expect(screen.getByRole("link", { name: "JOIN FREE" })).toHaveAttribute("href", expected);
  });
});

describe("StInvite, the second hero's clip", () => {
  afterEach(() => vi.unstubAllGlobals());

  // A window `width` px wide, with the clip's IntersectionObserver reporting `inView` once observed.
  const setup = (width: number, { inView = true, reduced = false } = {}) => {
    vi.stubGlobal("matchMedia", (query: string) => {
      const min = /min-width:\s*(\d+)px/.exec(query);
      const matches = query.includes("reduce") ? reduced : min ? width >= Number(min[1]) : false;
      return { matches, media: query, addEventListener() {}, removeEventListener() {} };
    });
    const observed: ((entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void)[] = [];
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(private readonly callback: (entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void) {}
        observe() {
          observed.push(this.callback);
        }
        disconnect() {}
      },
    );
    const { container } = render(<StInvite referrer={{ name: "bevynaut2", ethAddress: "0xABC" }} />);
    act(() => observed.forEach((callback) => callback([{ isIntersecting: inView, intersectionRatio: inView ? 0.5 : 0 }])));
    return container.querySelector("video")!;
  };

  it("should load and play the clip once it is in view on a desktop-wide screen", () => {
    const video = setup(1440);
    expect(video.getAttribute("src")).toMatch(/referral\.webm$/);
    expect(video.autoplay).toBe(true);
  });

  it("should keep the poster and load nothing below the 1024px desktop breakpoint", () => {
    const video = setup(1000);
    expect(video.hasAttribute("src")).toBe(false);
    expect(video.getAttribute("poster")).toMatch(/referral-poster\.webp$/);
    expect(video.getAttribute("preload")).toBe("none");
  });

  it("should load nothing for a visitor who prefers reduced motion", () => {
    expect(setup(1440, { reduced: true }).hasAttribute("src")).toBe(false);
  });

  it("should load nothing while the clip only touches the bottom edge of the viewport", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: !query.includes("reduce"), media: query, addEventListener() {}, removeEventListener() {} }));
    const observed: ((entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void)[] = [];
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(private readonly callback: (entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void) {}
        observe() {
          observed.push(this.callback);
        }
        disconnect() {}
      },
    );
    const { container } = render(<StInvite referrer={{ name: "bevynaut2", ethAddress: "0xABC" }} />);
    act(() => observed.forEach((callback) => callback([{ isIntersecting: true, intersectionRatio: 0 }])));
    expect(container.querySelector("video")!.hasAttribute("src")).toBe(false);
  });

  describe("when width or motion change after the clip has started", () => {
    // Live media queries: `set` changes one and notifies its listeners, as a browser does.
    const live = (width: number, reduced: boolean) => {
      const state = { width, reduced };
      const listeners = new Set<() => void>();
      vi.stubGlobal("matchMedia", (query: string) => ({
        get matches() {
          const min = /min-width:\s*(\d+)px/.exec(query);
          return query.includes("reduce") ? state.reduced : min ? state.width >= Number(min[1]) : false;
        },
        media: query,
        addEventListener: (_: string, listener: () => void) => listeners.add(listener),
        removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
      }));
      return (next: Partial<typeof state>) => {
        Object.assign(state, next);
        act(() => listeners.forEach((listener) => listener()));
      };
    };
    const observe = () => {
      const observed: ((entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void)[] = [];
      vi.stubGlobal(
        "IntersectionObserver",
        class {
          constructor(private readonly callback: (entries: { isIntersecting: boolean; intersectionRatio: number }[]) => void) {}
          observe() {
            observed.push(this.callback);
          }
          disconnect() {}
        },
      );
      return (inView: boolean) =>
        act(() => observed.forEach((callback) => callback([{ isIntersecting: inView, intersectionRatio: inView ? 0.5 : 0 }])));
    };
    const mount = () => {
      const { container } = render(<StInvite referrer={{ name: "bevynaut2", ethAddress: "0xABC" }} />);
      const video = container.querySelector("video")!;
      // As in a browser, the loaded resource (currentSrc) outlives a removed src attribute until load() runs.
      let current = "";
      const play = vi.fn(() => {
        current = video.getAttribute("src") ?? "";
        return Promise.resolve();
      });
      const pause = vi.fn();
      const load = vi.fn(() => {
        current = video.getAttribute("src") ?? "";
      });
      Object.assign(video, { play, pause, load });
      Object.defineProperty(video, "currentSrc", { get: () => current });
      return { video, play, pause, load };
    };

    it("should stop the clip and show the poster when reduced motion turns on", () => {
      const set = live(1440, false);
      const scroll = observe();
      const { video, pause, load } = mount();
      scroll(true);
      expect(video.getAttribute("src")).toMatch(/referral\.webm$/);
      set({ reduced: true });
      expect(video.hasAttribute("src")).toBe(false);
      expect(pause).toHaveBeenCalled();
      expect(load).toHaveBeenCalled();
    });

    it("should stop the clip when the window narrows below 1024px", () => {
      const set = live(1440, false);
      const scroll = observe();
      const { video, pause } = mount();
      scroll(true);
      set({ width: 900 });
      expect(video.hasAttribute("src")).toBe(false);
      expect(pause).toHaveBeenCalled();
    });

    it("should resume when motion is welcome again while the clip is in view", () => {
      const set = live(1440, false);
      const scroll = observe();
      const { video, play } = mount();
      scroll(true);
      set({ reduced: true });
      play.mockClear();
      set({ reduced: false });
      expect(video.getAttribute("src")).toMatch(/referral\.webm$/);
      expect(play).toHaveBeenCalled();
    });

    it("should stay on the poster when motion is welcome again after the clip left the view", () => {
      const set = live(1440, false);
      const scroll = observe();
      const { video } = mount();
      scroll(true);
      set({ reduced: true });
      scroll(false);
      set({ reduced: false });
      expect(video.hasAttribute("src")).toBe(false);
    });
  });

  it("should load nothing while the clip is out of view", () => {
    expect(setup(1440, { inView: false }).hasAttribute("src")).toBe(false);
  });
});
