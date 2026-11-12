// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UpcomingEventCardSlot } from "@dcl/one-ui/apps/sites/events/cards/UpcomingEventCardSlot";
import { ExperienceCardSlot } from "@dcl/one-ui/apps/sites/events/cards/ExperienceCardSlot";
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
          <UpcomingEventCardSlot {...props} />
        ) : (
          <ExperienceCardSlot {...props} live={false} />
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
  it("should toggle a reminder without opening the card", () => {
    const open = vi.fn();
    render(<UpcomingEventCardSlot {...props} onOpen={open} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Remind me: Poker Night" }),
    );
    expect(toggle).toHaveBeenCalledOnce();
    expect(open).not.toHaveBeenCalled();
  });
  it("should copy the actual card link without navigating", async () => {
    const open = vi.fn();
    render(<UpcomingEventCardSlot {...props} onOpen={open} />);
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
    render(<UpcomingEventCardSlot {...props} />);
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
    render(<UpcomingEventCardSlot {...props} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Copy link: Poker Night" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
    );
  });
  it("should recover when copying succeeds after a denied attempt", async () => {
    copy.mockRejectedValueOnce(new Error("denied"));
    render(<UpcomingEventCardSlot {...props} />);
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
