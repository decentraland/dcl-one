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

  it("should load nothing while the clip is out of view", () => {
    expect(setup(1440, { inView: false }).hasAttribute("src")).toBe(false);
  });
});
