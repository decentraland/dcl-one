// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { WeeklyRituals } from "@dcl/one-ui/apps/sites/home/WeeklyRituals";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";

import { track } from "@/core/lib/telemetry/track";
import { createLegacyTrackScope, trackLegacyClick } from "@/core/lib/telemetry/legacy-click";

import { homeClicks } from "./clicks";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));

const LABELS = { previous: "Previous", next: "Next", goTo: (n: number) => `Go to ${n}`, position: (n: number, t: number) => `${n} of ${t}` };
const SESSION = { sid: "sid-1", scope: createLegacyTrackScope() };
const clicks = () => homeClicks((click) => trackLegacyClick(click, SESSION));

/** The properties of every Click sent, without the audit timestamps. */
function sentClicks() {
  return vi.mocked(track).mock.calls.map(([event, props]) => {
    expect(event).toBe("Click");
    const { track_called_at: _called, track_delivered_at: _delivered, ...rest } = props as Record<string, unknown>;
    return rest;
  });
}

function blockNavigation(event: MouseEvent) {
  event.preventDefault();
}

beforeAll(() => {
  window.matchMedia ??= ((query: string) => ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {} })) as unknown as typeof window.matchMedia;
  globalThis.ResizeObserver ??= class {
    observe() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
beforeEach(() => {
  window.history.replaceState(null, "", "/");
  // jsdom cannot navigate; the anchors' default action is not under test.
  document.addEventListener("click", blockNavigation);
});
afterEach(() => {
  document.removeEventListener("click", blockNavigation);
  cleanup();
  vi.mocked(track).mockReset();
});

describe("What's On", () => {
  function renderWhatsOn() {
    const handlers = clicks();
    render(
      <WhatsOn
        title="What's On"
        cards={[{ id: "ev", title: "Rooftop Party", image: null, href: "/events?id=ev", jumpHref: "/play/?position=1,2", creator: { name: "Ann", href: "/profile/0xa" } }]}
        labels={{ live: "Live", by: "by", jumpIn: "Jump In" }}
        carouselLabels={LABELS}
        viewAll={{ label: "View All", href: "/events", onClick: handlers.viewAll }}
        onCardClick={handlers.whatsOnCard}
        onJumpInClick={handlers.whatsOnJumpIn}
      />,
    );
  }

  it("sends place Landing Explore and section view_all for View All", async () => {
    renderWhatsOn();
    await userEvent.click(screen.getByRole("link", { name: /View All/ }));
    expect(sentClicks()).toEqual([{ place: "Landing Explore", section: "view_all", track_deferred: false }]);
  });

  it("names the card by its title when its page is opened", async () => {
    renderWhatsOn();
    await userEvent.click(within(screen.getByRole("list")).getByRole("link", { name: "Rooftop Party" }));
    expect(sentClicks()).toEqual([{ place: "Landing Explore", card: "Rooftop Party", track_deferred: false }]);
  });

  it("adds section jump_in for the card's Jump In, and sends no card-open Click with it", async () => {
    renderWhatsOn();
    await userEvent.click(within(screen.getByRole("list")).getByRole("link", { name: /Jump In/ }));
    expect(sentClicks()).toEqual([{ place: "Landing Explore", card: "Rooftop Party", section: "jump_in", track_deferred: false }]);
  });

  it("sends nothing for the creator's profile link", async () => {
    renderWhatsOn();
    await userEvent.click(within(screen.getByRole("list")).getByRole("link", { name: "Ann" }));
    expect(track).not.toHaveBeenCalled();
  });
});

it("sends place Landing Catch The Vibe, with no other attribute, for Hang out now", async () => {
  render(
    <CatchTheVibe
      title="Clips"
      items={[]}
      labels={{ play: "Play", pause: "Pause", mute: "Mute", unmute: "Unmute" }}
      carouselLabels={LABELS}
      hangOut={{ label: "Hang out now", href: "/play/", onClick: clicks().hangOut }}
    />,
  );
  await userEvent.click(screen.getByRole("link", { name: /Hang out now/ }));
  expect(sentClicks()).toEqual([{ place: "Landing Catch The Vibe", track_deferred: false }]);
});

it("names a weekly ritual by its id", async () => {
  render(
    <WeeklyRituals
      title="Rituals"
      labels={LABELS}
      onCardClick={clicks().ritual}
      cards={[{ id: "takeover-tuesdays", title: "Takeover Tuesdays", href: "/events?id=t", image: "/t.webp", mobileImage: "/tm.webp" }]}
    />,
  );
  const [ritual] = screen.getAllByRole("link");
  await userEvent.click(ritual!);
  expect(sentClicks()).toEqual([{ place: "Landing Weekly Rituals", card: "takeover-tuesdays", track_deferred: false }]);
});
