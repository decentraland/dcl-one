// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DayCard, UpcomingCard } from "@dcl/one-ui/apps/sites/events/cards";
vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);
const toggle = vi.fn();
const copy = vi.fn();
const props = {
  id: "event",
  title: "Poker Night",
  image: null,
  href: "/events?id=event",
  creator: { name: "Host", href: "/profile/host" },
  time: { label: "Tomorrow", dateTime: "2026-11-13T12:00:00Z" },
  remind: { active: false, pending: false, onToggle: toggle },
  calendarHref: "https://calendar.google.com/calendar/render?action=TEMPLATE",
};
beforeEach(() => {
  toggle.mockReset();
  copy.mockReset().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", { clipboard: { writeText: copy } });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
describe("when event cards reveal their actions", () => {
  it.each(["upcoming", "day"])(
    "should keep the %s card time and all three independent actions mounted",
    (kind) => {
      render(
        kind === "upcoming" ? (
          <UpcomingCard {...props} />
        ) : (
          <DayCard {...props} live={false} />
        ),
      );
      expect(screen.getByText("Tomorrow").closest("time")?.dateTime).toBe(
        props.time.dateTime,
      );
      expect(
        screen.getByRole("button", { name: "Remind me: Poker Night" }),
      ).toBeDefined();
      expect(
        screen
          .getByRole("link", { name: "Add to calendar: Poker Night" })
          .getAttribute("href"),
      ).toBe(props.calendarHref);
      expect(
        screen
          .getByRole("button", { name: "Copy link: Poker Night" })
          .closest("a"),
      ).toBeNull();
    },
  );
  it("should render the upcoming card as the public EventCard small variant with icon-only actions", () => {
    render(<UpcomingCard {...props} />);
    const card = screen.getByRole("article");
    expect(card.className).toContain("small");
    expect(card.textContent).toContain("by Host");
    for (const name of ["Remind me: Poker Night", "Copy link: Poker Night"])
      expect(screen.getByRole("button", { name }).textContent).toBe("");
    expect(screen.queryByText("Remind me")).toBeNull();
  });
  it("should mark an active reminder as pressed and disable it while pending", () => {
    render(
      <UpcomingCard
        {...props}
        remind={{ active: true, pending: true, onToggle: toggle }}
      />,
    );
    const button = screen.getByRole("button", {
      name: "Remind me: Poker Night",
    }) as HTMLButtonElement;
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(toggle).not.toHaveBeenCalled();
  });
  it("should leave out Remind me and Add to calendar when the page offers neither", () => {
    render(
      <UpcomingCard {...props} remind={undefined} calendarHref={undefined} />,
    );
    expect(screen.queryByRole("button", { name: /Remind me/ })).toBeNull();
    expect(screen.queryByRole("link", { name: /Add to calendar/ })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    ).toBeDefined();
  });
  it("should show a live event's badges, JUMP IN and Copy link instead of Remind me", () => {
    const onJumpIn = vi.fn((event: { preventDefault: () => void }) =>
      event.preventDefault(),
    );
    const open = vi.fn();
    render(
      <UpcomingCard
        {...props}
        remind={undefined}
        live
        users={16}
        usersLabel="16 online"
        jumpIn={{ href: "/play?position=1,1", onJumpIn }}
        onOpen={open}
      />,
    );
    expect(screen.getByRole("article").className).toContain("isLive");
    expect(screen.getByText("LIVE")).toBeDefined();
    expect(screen.getByText("16 online")).toBeDefined();
    const jump = screen.getByRole("link", { name: "Jump in: Poker Night" });
    expect(jump.getAttribute("href")).toBe("/play?position=1,1");
    fireEvent.click(jump);
    expect(onJumpIn).toHaveBeenCalledOnce();
    expect(open).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /Remind me/ })).toBeNull();
    expect(screen.queryByRole("link", { name: /Add to calendar/ })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    ).toBeDefined();
  });
  it("should keep a live event without a jump target to Copy link, with no unknown count", () => {
    render(<UpcomingCard {...props} remind={undefined} live image={null} />);
    expect(screen.queryByRole("link", { name: /Jump in/ })).toBeNull();
    expect(screen.getByText("LIVE")).toBeDefined();
    expect(screen.queryByText(/online/)).toBeNull();
    expect(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    ).toBeDefined();
  });
  it("should render a card with no host and no image", () => {
    render(<UpcomingCard {...props} creator={undefined} image={null} />);
    expect(screen.getByRole("article").textContent).not.toContain("by ");
    expect(screen.getByRole("link", { name: "Poker Night" })).toBeDefined();
  });
  it("should toggle a reminder without opening the card", () => {
    const open = vi.fn();
    render(<UpcomingCard {...props} onOpen={open} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Remind me: Poker Night" }),
    );
    expect(toggle).toHaveBeenCalledOnce();
    expect(open).not.toHaveBeenCalled();
  });
  it("should copy the actual card link without navigating", async () => {
    const open = vi.fn();
    render(<UpcomingCard {...props} onOpen={open} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Link copied."),
    );
    expect(copy).toHaveBeenCalledWith(
      new URL(props.href, window.location.href).href,
    );
    expect(open).not.toHaveBeenCalled();
  });
  it("should report a clipboard failure without changing the reminder", async () => {
    copy.mockRejectedValue(new Error("denied"));
    render(<UpcomingCard {...props} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
    );
    expect(toggle).not.toHaveBeenCalled();
  });
  it("should handle a missing clipboard API", async () => {
    vi.stubGlobal("navigator", {});
    render(<UpcomingCard {...props} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
    );
  });
  it("should recover when copying succeeds after a denied attempt", async () => {
    copy.mockRejectedValueOnce(new Error("denied"));
    render(<UpcomingCard {...props} />);
    const button = screen.getByRole("button", {
      name: "Copy link: Poker Night",
    });
    fireEvent.click(button);
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
    );
    fireEvent.click(button);
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Link copied."),
    );
  });
});
