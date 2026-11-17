import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import StWhatSOn, { type WoUpcomingCard } from "./StWhatSOn";

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

describe("StWhatSOn, the Upcoming card's hover actions", () => {
  afterEach(() => vi.unstubAllGlobals());

  const renderUpcoming = (event: Partial<WoUpcomingCard>) => {
    vi.stubGlobal("matchMedia", (media: string) => ({ matches: false, media, addEventListener() {}, removeEventListener() {} }));
    const card: WoUpcomingCard = { id: "e0", name: "Event 0", creator: "Ada", time: "Today 6:00 PM", hue: 0, ...event };
    const { container } = render(<StWhatSOn chrome={false} upcoming={[card]} />);
    return container.querySelector<HTMLElement>(".wo-up__grid .wo-sm__actions")!;
  };

  it("should offer Remind me and Copy link, as the Figma card-hover does, and no Jump in to world", () => {
    const actions = renderUpcoming({ id: "e 1", jumpUrl: "https://example.com/play/?position=1,1" });
    expect(actions.textContent).not.toMatch(/jump in/i);
    expect(actions.querySelector('a[href="https://example.com/play/?position=1,1"]')).toBeNull();
    const remind = [...actions.querySelectorAll("a")].find((link) => link.textContent?.trim() === "Remind me");
    expect(remind?.getAttribute("href")).toBe("/whats-on/e%201");
    expect(actions.querySelector('button[aria-label="Copy link"]')).not.toBeNull();
  });

  it("should keep Remind me for an event the player already attends", () => {
    const actions = renderUpcoming({ attending: true });
    expect(actions.textContent?.trim()).toBe("Remind me");
  });

  it("should not throw when the clipboard is missing", () => {
    vi.stubGlobal("navigator", {});
    const actions = renderUpcoming({});
    expect(() => fireEvent.click(actions.querySelector('button[aria-label="Copy link"]')!)).not.toThrow();
  });

  it("should open the event page from the icon form of Remind me", () => {
    const assign = vi.fn();
    vi.stubGlobal("location", { ...window.location, assign });
    const actions = renderUpcoming({ id: "e 1" });
    fireEvent.click(actions.querySelector('button[aria-label="Remind me"]')!);
    expect(assign).toHaveBeenCalledWith("/whats-on/e%201");
  });
});
