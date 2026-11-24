// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";

import { PlacesLiveNow as LiveNow, PlaceCardSlot, type PlaceCardData } from "@dcl/one-ui/apps/sites";

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

let scrollBy: Mock;
let scrollIntoView: Mock;

beforeEach(() => {
  scrollBy = vi.fn();
  scrollIntoView = vi.fn();
  HTMLElement.prototype.scrollBy = scrollBy as never;
  Element.prototype.scrollIntoView = scrollIntoView as never;
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

function renderRail(count = 4) {
  const cards = Array.from({ length: count }, (_, i) => card(String(i + 1)));
  render(<LiveNow title="Live Now" cards={cards} labels={LABELS} cardLabels={CARD_LABELS} />);
  const group = screen.getByRole("group", { name: "Live Now" });
  return { group, rail: within(group).getByRole("list") };
}

describe("when Live Now has cards", () => {
  describe("and the row fits", () => {
    beforeEach(() => {
      renderRail();
    });

    it("should show no arrows", () => {
      expect(screen.queryByRole("button", { name: "More live places" })).toBeNull();
    });

    it("should mark the first dot current", () => {
      expect(screen.getByRole("button", { name: "Show Scene 1 (1)" }).getAttribute("aria-current")).toBe("true");
    });

    describe("and a dot is clicked", () => {
      beforeEach(async () => {
        await userEvent.setup().click(screen.getByRole("button", { name: "Show Scene 3 (3)" }));
      });

      it("should scroll to the card the dot names", () => {
        expect(scrollIntoView).toHaveBeenCalledOnce();
      });
    });
  });

  describe("and the row overflows", () => {
    let rail: HTMLElement;

    beforeEach(() => {
      ({ rail } = renderRail());
      layOut(rail, 0);
    });

    describe("and the rail is at the start", () => {
      it("should disable the previous arrow", () => {
        expect(screen.getByRole("button", { name: "Previous live places" })).toHaveProperty("disabled", true);
      });

      it("should enable the next arrow", () => {
        expect(screen.getByRole("button", { name: "More live places" })).toHaveProperty("disabled", false);
      });
    });

    describe("and the rail is scrolled to the end", () => {
      beforeEach(() => {
        layOut(rail, 600);
      });

      it("should enable the previous arrow", () => {
        expect(screen.getByRole("button", { name: "Previous live places" })).toHaveProperty("disabled", false);
      });

      it("should disable the next arrow", () => {
        expect(screen.getByRole("button", { name: "More live places" })).toHaveProperty("disabled", true);
      });
    });

    describe("and the focus is inside the carousel", () => {
      let user: ReturnType<typeof userEvent.setup>;

      beforeEach(async () => {
        user = userEvent.setup();
        screen.getByRole("link", { name: "Scene 1" }).focus();
        await user.keyboard("{ArrowRight}");
      });

      describe("and ArrowRight is pressed", () => {
        it("should move one card forward", () => {
          expect(scrollBy).toHaveBeenLastCalledWith(expect.objectContaining({ left: 300 }));
        });
      });

      describe("and ArrowLeft is pressed after it", () => {
        beforeEach(async () => {
          await user.keyboard("{ArrowLeft}");
        });

        it("should move one card back", () => {
          expect(scrollBy).toHaveBeenLastCalledWith(expect.objectContaining({ left: -300 }));
        });
      });
    });

    describe("and the next arrow is clicked", () => {
      beforeEach(async () => {
        await userEvent.setup().click(screen.getByRole("button", { name: "More live places" }));
      });

      it("should move one card forward", () => {
        expect(scrollBy).toHaveBeenCalledWith(expect.objectContaining({ left: 300 }));
      });
    });
  });

  describe("and it has a single card", () => {
    beforeEach(() => {
      renderRail(1);
    });

    it("should show no dots", () => {
      expect(screen.queryByRole("button", { name: /Show Scene/ })).toBeNull();
    });
  });
});

describe("when Live Now has no cards", () => {
  beforeEach(() => {
    render(<LiveNow title="Live Now" cards={[]} labels={LABELS} cardLabels={CARD_LABELS} />);
  });

  it("should render nothing", () => {
    expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
  });
});

describe("when a place card renders", () => {
  describe("and it has people, a LIVE event, the featured mark and a Jump In target", () => {
    let liveCard: PlaceCardData;

    beforeEach(() => {
      liveCard = card("a", { users: 3, usersLabel: "3 people in-world", live: true, featured: true });
      render(
        <ul>
          <PlaceCardSlot card={liveCard} labels={CARD_LABELS} />
        </ul>,
      );
    });

    it("should name the people count", () => {
      expect(screen.getByRole("img", { name: "3 people in-world" })).toBeTruthy();
    });

    it("should name the LIVE event", () => {
      expect(screen.getByText("Live")).toBeTruthy();
    });

    it("should name the featured mark", () => {
      expect(screen.getByText("Featured")).toBeTruthy();
    });

    it("should link Jump In to the target", () => {
      expect(screen.getByRole("link", { name: "Jump in: Scene a" }).getAttribute("href")).toBe("/play?position=a");
    });
  });

  describe("and one has no Jump In target and a creator without a profile, the other a creator with one", () => {
    let noTarget: PlaceCardData;
    let withProfile: PlaceCardData;

    beforeEach(() => {
      noTarget = card("a", { jumpHref: undefined, creator: { name: "Ann" } });
      withProfile = card("b", { creator: { name: "Bo", href: "/profile/0xb" } });
      render(
        <ul>
          <PlaceCardSlot card={noTarget} labels={CARD_LABELS} />
          <PlaceCardSlot card={withProfile} labels={CARD_LABELS} />
        </ul>,
      );
    });

    it("should leave Jump In out without a target", () => {
      expect(screen.queryByRole("link", { name: "Jump in: Scene a" })).toBeNull();
    });

    it("should not link a creator without a profile", () => {
      expect(screen.queryByRole("link", { name: "Ann" })).toBeNull();
    });

    it("should link a creator with a profile", () => {
      expect(screen.getByRole("link", { name: "Bo" }).getAttribute("href")).toBe("/profile/0xb");
    });
  });

  describe("and it is a skeleton", () => {
    let container: HTMLElement;

    beforeEach(() => {
      ({ container } = render(
        <ul>
          <PlaceCardSlot skeleton />
        </ul>,
      ));
    });

    it("should hide it from assistive technology", () => {
      expect(container.querySelector("li")?.getAttribute("aria-hidden")).toBe("true");
    });
  });
});
