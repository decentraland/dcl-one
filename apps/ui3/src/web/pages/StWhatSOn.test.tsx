import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import StWhatSOn from "./StWhatSOn";

const upcoming = Array.from({ length: 6 }, (_, i) => ({ id: `e${i}`, name: `Event ${i}`, creator: "Ada", time: "Today 6:00 PM", hue: 0 }));

describe("StWhatSOn, the Upcoming rail below the desktop grid", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("scrolls its pages sideways", () => {
    const css = readFileSync(join(__dirname, "stwhatson.css"), "utf8");
    const rule = /\.wo-up__track \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toContain("overflow-x: auto");
    expect(rule).toContain("scroll-snap-type: x mandatory");
  });

  it("marks the page in view on its dots", () => {
    vi.stubGlobal("matchMedia", (media: string) => ({ matches: false, media, addEventListener() {}, removeEventListener() {} }));
    const { container } = render(<StWhatSOn chrome={false} upcoming={upcoming} />);
    const track = container.querySelector<HTMLElement>(".wo-up__track")!;
    const active = () => [...container.querySelectorAll(".wo-up__dots .wo-dots__dot")].map((dot) => dot.classList.contains("is-active"));
    expect(track.children).toHaveLength(2);
    expect(active()).toEqual([true, false]);

    Object.defineProperties(track, { clientWidth: { value: 320 }, scrollLeft: { value: 320 } });
    fireEvent.scroll(track);

    expect(active()).toEqual([false, true]);
  });
});
