// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LiveNow } from "@dcl/one-ui/apps/sites/places/LiveNow";
import { PlaceCardSlot } from "@dcl/one-ui/apps/sites/places/cards/PlaceCardSlot";
import type { PlaceCardData } from "@dcl/one-ui/apps/sites/places/cards/types";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const CARD_LABELS = { by: "By", jumpIn: "Jump in", featured: "Featured", live: "Live" };
const LABELS = { previous: "Previous live places", next: "More live places", goTo: (n: number, title: string) => `Show ${title} (${n})` };

function card(id: string, extra: Partial<PlaceCardData> = {}): PlaceCardData {
  return { id, title: `Scene ${id}`, image: null, href: `/places/${id}`, jumpHref: `/play?position=${id}`, ...extra };
}

/** jsdom lays nothing out: give the rail the geometry of a row twice as wide as its box. */
function layOut(rail: HTMLElement, scrollLeft: number) {
  Object.defineProperties(rail, {
    clientWidth: { configurable: true, value: 600 },
    scrollWidth: { configurable: true, value: 1200 },
    scrollLeft: { configurable: true, value: scrollLeft },
  });
  fireEvent.scroll(rail);
}

let scrollBy: ReturnType<typeof vi.fn>;
let scrollIntoView: ReturnType<typeof vi.fn>;

beforeEach(() => {
  scrollBy = vi.fn();
  scrollIntoView = vi.fn();
  HTMLElement.prototype.scrollBy = scrollBy as never;
  Element.prototype.scrollIntoView = scrollIntoView as never;
});

afterEach(cleanup);

function renderRail(count = 4) {
  const cards = Array.from({ length: count }, (_, i) => card(String(i + 1)));
  render(<LiveNow title="Live Now" cards={cards} labels={LABELS} cardLabels={CARD_LABELS} />);
  const group = screen.getByRole("group", { name: "Live Now" });
  return { group, rail: within(group).getByRole("list") };
}

describe("when Live Now has cards", () => {
  it("should show no arrows while the row fits", () => {
    renderRail();
    expect(screen.queryByRole("button", { name: "More live places" })).toBeNull();
  });

  it("should disable the previous arrow at the start and the next arrow at the end", () => {
    const { rail } = renderRail();
    layOut(rail, 0);
    expect(screen.getByRole("button", { name: "Previous live places" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "More live places" })).toHaveProperty("disabled", false);
    layOut(rail, 600);
    expect(screen.getByRole("button", { name: "Previous live places" })).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "More live places" })).toHaveProperty("disabled", true);
  });

  it("should move one card per arrow key from inside the carousel", async () => {
    const { rail } = renderRail();
    layOut(rail, 0);
    const user = userEvent.setup();
    screen.getByRole("link", { name: "Scene 1" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(scrollBy).toHaveBeenLastCalledWith(expect.objectContaining({ left: 300 }));
    await user.keyboard("{ArrowLeft}");
    expect(scrollBy).toHaveBeenLastCalledWith(expect.objectContaining({ left: -300 }));
  });

  it("should move with the arrow buttons", async () => {
    const { rail } = renderRail();
    layOut(rail, 0);
    await userEvent.setup().click(screen.getByRole("button", { name: "More live places" }));
    expect(scrollBy).toHaveBeenCalledWith(expect.objectContaining({ left: 300 }));
  });

  it("should mark the first dot current and scroll to the card a dot names", async () => {
    renderRail();
    expect(screen.getByRole("button", { name: "Show Scene 1 (1)" }).getAttribute("aria-current")).toBe("true");
    await userEvent.setup().click(screen.getByRole("button", { name: "Show Scene 3 (3)" }));
    expect(scrollIntoView).toHaveBeenCalledOnce();
  });

  it("should show no dots for a single card", () => {
    renderRail(1);
    expect(screen.queryByRole("button", { name: /Show Scene/ })).toBeNull();
  });

  it("should render nothing without cards", () => {
    render(<LiveNow title="Live Now" cards={[]} labels={LABELS} cardLabels={CARD_LABELS} />);
    expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
  });
});

describe("when a place card renders", () => {
  it("should name the people count, the LIVE event and Jump In", () => {
    render(
      <ul>
        <PlaceCardSlot card={card("a", { users: 3, usersLabel: "3 people in-world", live: true, featured: true })} labels={CARD_LABELS} />
      </ul>,
    );
    expect(screen.getByRole("img", { name: "3 people in-world" })).toBeTruthy();
    expect(screen.getByText("Live")).toBeTruthy();
    expect(screen.getByText("Featured")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Jump in: Scene a" }).getAttribute("href")).toBe("/play?position=a");
  });

  it("should leave Jump In out without a target and link the creator only with a profile", () => {
    render(
      <ul>
        <PlaceCardSlot card={card("a", { jumpHref: undefined, creator: { name: "Ann" } })} labels={CARD_LABELS} />
        <PlaceCardSlot card={card("b", { creator: { name: "Bo", href: "/profile/0xb" } })} labels={CARD_LABELS} />
      </ul>,
    );
    expect(screen.queryByRole("link", { name: "Jump in: Scene a" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Ann" })).toBeNull();
    expect(screen.getByRole("link", { name: "Bo" }).getAttribute("href")).toBe("/profile/0xb");
  });

  it("should hide a skeleton from assistive technology", () => {
    const { container } = render(
      <ul>
        <PlaceCardSlot skeleton />
      </ul>,
    );
    expect(container.querySelector("li")?.getAttribute("aria-hidden")).toBe("true");
  });
});
