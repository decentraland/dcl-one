import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import HeroVideo from "./HeroVideo";

function mockMedia(matches: Record<string, boolean>) {
  window.matchMedia = ((q: string) => ({ matches: matches[q] ?? false })) as unknown as typeof window.matchMedia;
}

describe("HeroVideo", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("mounts a muted looping video on desktop once idle", () => {
    mockMedia({ "(min-width: 900px)": true });
    const { container } = render(<HeroVideo />);
    expect(container.querySelector("video")).toBeNull();
    act(() => void vi.advanceTimersByTime(5000));
    const video = container.querySelector("video")!;
    expect(video).not.toBeNull();
    expect(video.loop).toBe(true);
    expect(video.muted).toBe(true);
    expect(video.querySelectorAll("source")).toHaveLength(2);
  });

  it("never mounts on narrow viewports", () => {
    mockMedia({});
    const { container } = render(<HeroVideo />);
    act(() => void vi.advanceTimersByTime(5000));
    expect(container.querySelector("video")).toBeNull();
  });

  it("never mounts when reduced motion is requested", () => {
    mockMedia({ "(min-width: 900px)": true, "(prefers-reduced-motion: reduce)": true });
    const { container } = render(<HeroVideo />);
    act(() => void vi.advanceTimersByTime(5000));
    expect(container.querySelector("video")).toBeNull();
  });
});
