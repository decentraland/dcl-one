// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { DayCard, UpcomingCard } from "@dcl/one-ui";
vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);
type JumpIn = (event: { preventDefault: () => void }) => void;

let toggle: Mock<() => void>;
let copy: Mock<(text: string) => Promise<void>>;
let open: Mock<() => void>;
let onJumpIn: Mock<JumpIn>;
let props: ComponentProps<typeof UpcomingCard>;
let user: ReturnType<typeof userEvent.setup>;
beforeEach(() => {
  // Set up before the clipboard stub below, so the stub, not user-event's own clipboard, is what the card writes to.
  user = userEvent.setup();
  toggle = vi.fn();
  copy = vi.fn().mockResolvedValue(undefined);
  open = vi.fn();
  onJumpIn = vi.fn((event: { preventDefault: () => void }) =>
    event.preventDefault(),
  );
  props = {
    id: "event",
    title: "Poker Night",
    image: null,
    href: "/events?id=event",
    creator: { name: "Host", href: "/profile/host" },
    time: { label: "Tomorrow", dateTime: "2026-11-13T12:00:00Z" },
    remind: { active: false, pending: false, onToggle: toggle },
    calendarHref: "https://calendar.google.com/calendar/render?action=TEMPLATE",
  };
  vi.stubGlobal("navigator", { clipboard: { writeText: copy } });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});
describe("when event cards reveal their actions", () => {
  describe.each(["upcoming", "day"])("and the card is the %s card", (kind) => {
    beforeEach(() => {
      render(
        kind === "upcoming" ? (
          <UpcomingCard {...props} />
        ) : (
          <DayCard {...props} live={false} />
        ),
      );
    });
    it("should keep the card time mounted", () => {
      expect(screen.getByText("Tomorrow").closest("time")?.dateTime).toBe(
        props.time?.dateTime,
      );
    });
    it("should keep Remind me mounted", () => {
      expect(
        screen.getByRole("button", { name: "Remind me: Poker Night" }),
      ).toBeDefined();
    });
    it("should keep Add to calendar mounted with the calendar link", () => {
      expect(
        screen
          .getByRole("link", { name: "Add to calendar: Poker Night" })
          .getAttribute("href"),
      ).toBe(props.calendarHref);
    });
    it("should keep Copy link as an independent button outside the card link", () => {
      expect(
        screen
          .getByRole("button", { name: "Copy link: Poker Night" })
          .closest("a"),
      ).toBeNull();
    });
  });
  describe("and the card is the upcoming card", () => {
    it("should render it as the public EventCard small variant with icon-only actions", () => {
      render(<UpcomingCard {...props} />);
      const card = screen.getByRole("article");
      expect(card.className).toContain("small");
      expect(card.textContent).toContain("by Host");
      for (const name of ["Remind me: Poker Night", "Copy link: Poker Night"])
        expect(screen.getByRole("button", { name }).textContent).toBe("");
      expect(screen.queryByText("Remind me")).toBeNull();
    });
  });
  describe("and the reminder is active and pending", () => {
    let button: HTMLButtonElement;
    beforeEach(() => {
      render(
        <UpcomingCard
          {...props}
          remind={{ active: true, pending: true, onToggle: toggle }}
        />,
      );
      button = screen.getByRole("button", {
        name: "Remind me: Poker Night",
      }) as HTMLButtonElement;
    });
    it("should mark the reminder as pressed", () => {
      expect(button.getAttribute("aria-pressed")).toBe("true");
    });
    it("should disable the reminder so a click does not toggle it", async () => {
      expect(button.disabled).toBe(true);
      await user.click(button);
      expect(toggle).not.toHaveBeenCalled();
    });
  });
  describe("and the page offers neither a reminder nor a calendar link", () => {
    beforeEach(() => {
      render(
        <UpcomingCard {...props} remind={undefined} calendarHref={undefined} />,
      );
    });
    it("should leave out Remind me", () => {
      expect(screen.queryByRole("button", { name: /Remind me/ })).toBeNull();
    });
    it("should leave out Add to calendar", () => {
      expect(screen.queryByRole("link", { name: /Add to calendar/ })).toBeNull();
    });
    it("should still offer Copy link", () => {
      expect(
        screen.getByRole("button", { name: "Copy link: Poker Night" }),
      ).toBeDefined();
    });
  });
  describe("and the event is live with a jump target", () => {
    beforeEach(() => {
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
    });
    it("should show the live badges", () => {
      expect(screen.getByRole("article").className).toContain("isLive");
      expect(screen.getByText("LIVE")).toBeDefined();
      expect(screen.getByText("16 online")).toBeDefined();
    });
    it("should jump in through JUMP IN without opening the card", async () => {
      const jump = screen.getByRole("link", { name: "Jump in: Poker Night" });
      expect(jump.getAttribute("href")).toBe("/play?position=1,1");
      await user.click(jump);
      expect(onJumpIn).toHaveBeenCalledOnce();
      expect(open).not.toHaveBeenCalled();
    });
    it("should show Copy link instead of Remind me and Add to calendar", () => {
      expect(screen.queryByRole("button", { name: /Remind me/ })).toBeNull();
      expect(screen.queryByRole("link", { name: /Add to calendar/ })).toBeNull();
      expect(
        screen.getByRole("button", { name: "Copy link: Poker Night" }),
      ).toBeDefined();
    });
  });
  describe("and a live event is in a desktop day column", () => {
    const href = "/play?realm=gallery.dcl.eth";

    describe("and it has a launch destination", () => {
      it("should expose Jump In independently of the detail link", async () => {
        render(
          <DayCard
            {...props}
            live
            users={0}
            jumpIn={{ href, onJumpIn }}
            onOpen={open}
          />,
        );
        const link = screen.getByRole("link", { name: "Jump in: Poker Night" });
        expect(link.getAttribute("href")).toBe(href);
        await user.click(link);
        expect(onJumpIn).toHaveBeenCalledOnce();
        expect(open).not.toHaveBeenCalled();
      });
    });

    describe("and no launch destination was supplied", () => {
      beforeEach(() => {
        render(<DayCard {...props} live />);
      });
      it("should not offer Jump in", () => {
        expect(screen.queryByRole("link", { name: /Jump in/ })).toBeNull();
      });
      it("should offer Copy link", () => {
        expect(
          screen.getByRole("button", { name: "Copy link: Poker Night" }),
        ).toBeDefined();
      });
    });
  });
  describe("and a live event has no jump target and no player count", () => {
    beforeEach(() => {
      render(<UpcomingCard {...props} remind={undefined} live image={null} />);
    });
    it("should not offer Jump in", () => {
      expect(screen.queryByRole("link", { name: /Jump in/ })).toBeNull();
    });
    it("should show the LIVE badge without an unknown count", () => {
      expect(screen.getByText("LIVE")).toBeDefined();
      expect(screen.queryByText(/online/)).toBeNull();
    });
    it("should keep Copy link", () => {
      expect(
        screen.getByRole("button", { name: "Copy link: Poker Night" }),
      ).toBeDefined();
    });
  });
  describe("and the card has no host and no image", () => {
    beforeEach(() => {
      render(<UpcomingCard {...props} creator={undefined} image={null} />);
    });
    it("should leave out the by-line", () => {
      expect(screen.getByRole("article").textContent).not.toContain("by ");
    });
    it("should still link the card", () => {
      expect(screen.getByRole("link", { name: "Poker Night" })).toBeDefined();
    });
  });
  describe("and Remind me is clicked", () => {
    beforeEach(async () => {
      render(<UpcomingCard {...props} onOpen={open} />);
      await user.click(
        screen.getByRole("button", { name: "Remind me: Poker Night" }),
      );
    });
    it("should toggle the reminder", () => {
      expect(toggle).toHaveBeenCalledOnce();
    });
    it("should not open the card", () => {
      expect(open).not.toHaveBeenCalled();
    });
  });
  describe("and Copy link is clicked", () => {
    describe("and the clipboard accepts the link", () => {
      beforeEach(async () => {
        render(<UpcomingCard {...props} onOpen={open} />);
        await user.click(
          screen.getByRole("button", { name: "Copy link: Poker Night" }),
        );
      });
      it("should announce that the link was copied", async () => {
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toBe("Copied"),
        );
      });
      it("should describe the Copy link button with the visible confirmation", async () => {
        const tooltip = await screen.findByRole("tooltip", { name: "Copied" });
        expect(screen.getByRole("button", { name: "Copy link: Poker Night" }).getAttribute("aria-describedby")).toBe(tooltip.id);
      });
      it("should copy the actual card link without navigating", async () => {
        await waitFor(() => expect(copy).toHaveBeenCalled());
        expect(copy).toHaveBeenCalledWith(
          new URL(props.href, window.location.href).href,
        );
        expect(open).not.toHaveBeenCalled();
      });
    });
    describe("and the clipboard denies the write", () => {
      beforeEach(async () => {
        copy.mockRejectedValue(new Error("denied"));
        render(<UpcomingCard {...props} />);
        await user.click(
          screen.getByRole("button", { name: "Copy link: Poker Night" }),
        );
      });
      it("should report the clipboard failure", async () => {
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
        );
      });
      it("should not change the reminder", async () => {
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
        );
        expect(toggle).not.toHaveBeenCalled();
      });
    });
    describe("and the clipboard API is missing", () => {
      beforeEach(async () => {
        vi.stubGlobal("navigator", {});
        render(<UpcomingCard {...props} />);
        await user.click(
          screen.getByRole("button", { name: "Copy link: Poker Night" }),
        );
      });
      it("should report the failure", async () => {
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
        );
      });
    });
    describe("and a denied attempt is followed by one that succeeds", () => {
      beforeEach(() => {
        copy.mockRejectedValueOnce(new Error("denied"));
      });
      it("should recover and announce that the link was copied", async () => {
        render(<UpcomingCard {...props} />);
        const button = screen.getByRole("button", {
          name: "Copy link: Poker Night",
        });
        await user.click(button);
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toContain("Couldn't copy"),
        );
        await user.click(button);
        await waitFor(() =>
          expect(screen.getByRole("status").textContent).toBe("Copied"),
        );
      });
    });
  });
});
