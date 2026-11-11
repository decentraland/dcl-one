// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { JumpCard } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpEventActions } from "@dcl/one-ui/apps/sites/jump/JumpEventActions";
import { JumpInvalid } from "@dcl/one-ui/apps/sites/jump/JumpInvalid";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

afterEach(cleanup);

const base = {
  kind: "place" as const,
  title: "Plaza",
  creator: { name: "Studio" },
  location: { parcel: "10, 20" },
};

describe("JumpCard", () => {
  it("should link Jump In to the travel target", () => {
    render(<JumpCard {...base} jumpInHref="/play/?position=10%2C20" />);
    expect(screen.getByRole("heading", { name: "Plaza" })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Jump In/ }).getAttribute("href")).toBe("/play/?position=10%2C20");
  });

  it("should link the creator to their profile only when an address is known", () => {
    const { rerender } = render(<JumpCard {...base} creator={{ name: "Studio", href: "/profile/0xab" }} />);
    expect(screen.getByRole("link", { name: "View Studio's profile" }).getAttribute("href")).toBe("/profile/0xab");
    rerender(<JumpCard {...base} />);
    expect(screen.queryByRole("link", { name: /profile/ })).toBeNull();
  });

  it("should fall back to the default description", () => {
    render(<JumpCard {...base} />);
    expect(screen.getByText(/You're about to jump into Decentraland!/)).toBeTruthy();
  });

  it("should label a World location", () => {
    render(<JumpCard {...base} location={{ world: "foo.dcl.eth", parcel: "0, 0" }} />);
    expect(screen.getByLabelText("World: foo.dcl.eth")).toBeTruthy();
  });

  it("should render the actions instead of Jump In", () => {
    render(<JumpCard {...base} jumpInHref="/play/" actions={<button type="button">Custom</button>} />);
    expect(screen.getByRole("button", { name: "Custom" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Jump In/ })).toBeNull();
  });

  it.each([
    [{ kind: "live", count: 4 } as const, "LIVE +4"],
    [{ kind: "attendees", count: 9 } as const, "+9"],
    [{ kind: "players", count: 2 } as const, "2"],
  ])("should show the %o badge", (badge, text) => {
    render(<JumpCard {...base} badge={badge} />);
    expect(screen.getByText(text)).toBeTruthy();
  });
});

describe("JumpEventActions", () => {
  it("should toggle Remind me and expose its pressed state", () => {
    const onToggle = vi.fn();
    render(
      <JumpEventActions
        state="upcoming"
        calendarHref="https://calendar.google.com/x"
        remind={{ active: true, pending: false, onToggle }}
        onShare={vi.fn()}
      />,
    );
    const remind = screen.getByRole("button", { name: "Remind me" });
    expect(remind.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(remind);
    expect(onToggle).toHaveBeenCalledOnce();
    expect(screen.getByRole("link", { name: "Add to calendar" }).getAttribute("target")).toBe("_blank");
  });

  it("should disable Remind me while it is pending and omit calendar without a link", () => {
    render(<JumpEventActions state="upcoming" remind={{ active: false, pending: true, onToggle: vi.fn() }} onShare={vi.fn()} />);
    expect((screen.getByRole("button", { name: "Remind me" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole("link", { name: "Add to calendar" })).toBeNull();
  });

  it("should share and announce the notice", () => {
    const onShare = vi.fn();
    render(
      <JumpEventActions
        state="upcoming"
        remind={{ active: false, pending: false, onToggle: vi.fn() }}
        onShare={onShare}
        shareNotice="Link copied to clipboard."
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Share" }));
    expect(onShare).toHaveBeenCalledOnce();
    expect(screen.getByRole("status").textContent).toBe("Link copied to clipboard.");
  });

  it("should point an ended event to the events list", () => {
    render(<JumpEventActions state="ended" eventsHref="/whats-on" />);
    expect(screen.getByRole("link", { name: "Explore events" }).getAttribute("href")).toBe("/whats-on");
  });

  it("should tell a deleted event is gone", () => {
    render(<JumpEventActions state="deleted" eventsHref="/whats-on" />);
    expect(screen.getByText(/This event is no longer available/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "All Events" }).getAttribute("href")).toBe("/whats-on");
  });
});

describe("JumpInvalid", () => {
  const hrefs = { jumpInHref: "/play/", eventsHref: "/whats-on", homeHref: "/" };

  it("should offer events only on an event link", () => {
    const { rerender } = render(<JumpInvalid kind="place" {...hrefs} />);
    expect(screen.queryByRole("link", { name: "Explore events" })).toBeNull();
    rerender(<JumpInvalid kind="event" {...hrefs} />);
    expect(screen.getByRole("link", { name: "Explore events" })).toBeTruthy();
  });

  it("should offer Jump In and the homepage", () => {
    render(<JumpInvalid kind="place" {...hrefs} />);
    expect(screen.getByRole("heading", { name: "Oops!" })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Jump into Decentraland/ }).getAttribute("href")).toBe("/play/");
    expect(screen.getByRole("link", { name: /Go to Homepage/ }).getAttribute("href")).toBe("/");
  });
});
