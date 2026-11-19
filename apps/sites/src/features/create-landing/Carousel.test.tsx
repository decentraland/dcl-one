// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";

import { Carousel } from "./Carousel";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

// With reduced motion a step runs with no transition, as it does before the track is measured.
let reducedMotion = true;

beforeAll(() => {
  window.matchMedia = ((query: string) => ({ matches: reducedMotion && query.includes("prefers-reduced-motion"), media: query, addEventListener: () => {}, removeEventListener: () => {} })) as unknown as typeof window.matchMedia;
  globalThis.ResizeObserver = class {
    constructor(private readonly report: (entries: Array<{ contentRect: { width: number } }>) => void) {}
    observe() {
      this.report([{ contentRect: { width: 1_000 } }]);
    }
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  reducedMotion = true;
});

const items = ["one", "two", "three"];
const carousel = (autoplayDelay = 0) => (
  <Carousel label="Cards" items={items} keyOf={(item) => item} autoplayDelay={autoplayDelay} renderItem={(item) => <a href={`/${item}`}>{item}</a>} />
);
/** The link a keyboard and a screen reader can reach. */
const reachable = () => screen.getAllByRole("link").map((link) => link.textContent);

it("names the region and leaves only the middle slide to focus and to assistive technology", () => {
  const { container } = render(carousel());
  expect(screen.getByRole("region", { name: "Cards" })).toBeTruthy();
  expect(container.querySelectorAll("a")).toHaveLength(9);
  expect(container.querySelectorAll("[inert] a")).toHaveLength(8);
  expect(container.querySelectorAll("[aria-hidden='true'] a")).toHaveLength(8);
  expect(reachable()).toEqual(["one"]);
});

it("renders nothing without items", () => {
  const { container } = render(<Carousel label="Cards" items={[]} keyOf={String} renderItem={() => null} />);
  expect(container.innerHTML).toBe("");
});

it("steps with no transition to wait for, round the end and back, and never stops answering", () => {
  render(carousel());
  const next = screen.getByRole("button", { name: "Next slide" });
  const seen: string[][] = [];
  for (let i = 0; i < 4; i++) {
    fireEvent.click(next);
    seen.push(reachable());
  }
  expect(seen).toEqual([["two"], ["three"], ["one"], ["two"]]);
  fireEvent.click(screen.getByRole("button", { name: "Previous slide" }));
  fireEvent.click(screen.getByRole("button", { name: "Go to slide 3" }));
  expect(reachable()).toEqual(["three"]);
});

it("the arrow keys step from the carousel's controls, and stay with a slide's own controls", () => {
  render(carousel());
  fireEvent.keyDown(screen.getByRole("button", { name: "Go to slide 1" }), { key: "ArrowRight" });
  expect(reachable()).toEqual(["two"]);
  fireEvent.keyDown(screen.getByRole("link"), { key: "ArrowRight" });
  expect(reachable()).toEqual(["two"]);
  fireEvent.keyDown(screen.getByRole("button", { name: "Next slide" }), { key: "ArrowLeft" });
  expect(reachable()).toEqual(["one"]);
});

it("autoplay holds under the pointer and with focus inside, and ends when the visitor steps", () => {
  vi.useFakeTimers();
  reducedMotion = false;
  const { container } = render(carousel(1_000));
  const region = screen.getByRole("region", { name: "Cards" });
  const track = container.querySelector("[aria-live]")!;
  expect(track.getAttribute("aria-live")).toBe("off");
  act(() => void vi.advanceTimersByTime(1_000));
  expect(reachable()).toEqual(["two"]);
  fireEvent.transitionEnd(track);

  fireEvent.mouseEnter(region);
  act(() => void vi.advanceTimersByTime(3_000));
  expect(reachable()).toEqual(["two"]);
  fireEvent.mouseLeave(region);
  fireEvent.focus(screen.getByRole("link"));
  act(() => void vi.advanceTimersByTime(3_000));
  expect(reachable()).toEqual(["two"]);
  fireEvent.blur(screen.getByRole("link"));
  act(() => void vi.advanceTimersByTime(1_000));
  expect(reachable()).toEqual(["three"]);
  fireEvent.transitionEnd(track);

  fireEvent.click(screen.getByRole("button", { name: "Next slide" }));
  expect(reachable()).toEqual(["one"]);
  fireEvent.transitionEnd(track);
  act(() => void vi.advanceTimersByTime(5_000));
  expect(reachable()).toEqual(["one"]);
  expect(track.getAttribute("aria-live")).toBe("polite");
});

/** A carousel that slides: motion is accepted and the track is measured. */
function slidingCarousel() {
  vi.useFakeTimers();
  reducedMotion = false;
  const { container } = render(carousel());
  act(() => void vi.advanceTimersByTime(100));
  return { track: container.querySelector("[aria-live]")!, next: screen.getByRole("button", { name: "Next slide" }) };
}

it("a press on the track while a slide moves does not stop the controls", () => {
  const { track, next } = slidingCarousel();
  fireEvent.click(next);
  expect(reachable()).toEqual(["two"]);
  // The press takes the transition off the track, so the browser sends no transitionend for that step.
  fireEvent.mouseDown(track, { button: 0, clientX: 100 });
  fireEvent.mouseUp(window);
  fireEvent.click(next);
  expect(reachable()).toEqual(["three"]);
  fireEvent.click(screen.getByRole("button", { name: "Go to slide 1" }));
  expect(reachable()).toEqual(["three"]);
  fireEvent.transitionEnd(track);
  fireEvent.click(screen.getByRole("button", { name: "Go to slide 1" }));
  expect(reachable()).toEqual(["one"]);
});

it("a step whose transition never ends does not stop the controls, and the track comes back to the middle copy", () => {
  const { next } = slidingCarousel();
  const seen: string[][] = [];
  for (let i = 0; i < 7; i++) {
    fireEvent.click(next);
    seen.push(reachable());
    // No transitionend comes, as in a tab the browser does not draw.
    act(() => void vi.advanceTimersByTime(600));
  }
  expect(seen).toEqual([["two"], ["three"], ["one"], ["two"], ["three"], ["one"], ["two"]]);
});
