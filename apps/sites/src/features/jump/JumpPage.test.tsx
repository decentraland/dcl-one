// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ReactNode, StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, type Mock, type MockInstance, vi } from "vitest";

import { JumpCard, type JumpCardProps } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpEventActions } from "@dcl/one-ui/apps/sites/jump/JumpEventActions";
import { JumpInvalid } from "@dcl/one-ui/apps/sites/jump/JumpInvalid";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

afterEach(cleanup);

afterEach(() => {
  vi.restoreAllMocks();
});

// A stand-in WebGL context: every call succeeds, and the cleanup's context release is recorded.
function fakeContext(loseContext: Mock) {
  return new Proxy({} as Record<string, unknown>, {
    get: (target, key) => {
      if (key in target) return target[key as string];
      if (key === "getExtension") return () => ({ loseContext });
      if (key === "getShaderParameter" || key === "getProgramParameter") return () => true;
      if (typeof key === "string" && /^[A-Z_0-9]+$/.test(key)) return 0;
      return () => ({});
    },
  });
}

describe("JumpCard", () => {
  let base: Pick<JumpCardProps, "kind" | "title" | "creator" | "location">;

  beforeEach(() => {
    base = {
      kind: "place" as const,
      title: "Plaza",
      creator: { name: "Studio" },
      location: { parcel: "10, 20" },
    };
  });

  describe("when it has a travel target", () => {
    beforeEach(() => {
      render(<JumpCard {...base} jumpInHref="/play/?position=10%2C20" />);
    });

    it("should show the title as a heading", () => {
      expect(screen.getByRole("heading", { name: "Plaza" })).toBeTruthy();
    });

    it("should link Jump In to the travel target", () => {
      expect(screen.getByRole("link", { name: /Jump In/ }).getAttribute("href")).toBe("/play/?position=10%2C20");
    });
  });

  describe("when it has a Jump In handler and Jump In is clicked", () => {
    let onJumpIn: Mock<(event: { preventDefault(): void }) => void>;

    beforeEach(async () => {
      onJumpIn = vi.fn((event: { preventDefault(): void }) => event.preventDefault());
      render(<JumpCard {...base} jumpInHref="decentraland://open" onJumpIn={onJumpIn} />);
      await userEvent.setup().click(screen.getByRole("link", { name: /Jump In/ }));
    });

    it("should run the Jump In handler", () => {
      expect(onJumpIn).toHaveBeenCalledOnce();
    });
  });

  describe("when the creator's address is known", () => {
    let rerender: (ui: ReactNode) => void;

    beforeEach(() => {
      ({ rerender } = render(<JumpCard {...base} creator={{ name: "Studio", href: "/profile/0xab" }} />));
    });

    it("should link the creator to their profile", () => {
      expect(screen.getByRole("link", { name: "View Studio's profile" }).getAttribute("href")).toBe("/profile/0xab");
    });

    describe("and the card is re-rendered without it", () => {
      beforeEach(() => {
        rerender(<JumpCard {...base} />);
      });

      it("should not link any profile", () => {
        expect(screen.queryByRole("link", { name: /profile/ })).toBeNull();
      });
    });
  });

  describe("when the creator has a colour", () => {
    beforeEach(() => {
      render(<JumpCard {...base} creator={{ name: "Studio", color: "#123456" }} />);
    });

    it("should tint the disc behind the avatar with the creator's colour", () => {
      const avatar = screen.getByRole("heading", { name: "Plaza" }).parentElement?.querySelector("img");
      expect(avatar?.style.getPropertyValue("--avatar-bg")).toBe("#123456");
    });
  });

  describe("when the title is empty", () => {
    beforeEach(() => {
      render(<JumpCard {...base} title="" />);
    });

    it("should describe the placeholder image", () => {
      expect(screen.getByRole("img", { name: "Place image" })).toBeTruthy();
    });
  });

  describe("when no description is given", () => {
    beforeEach(() => {
      render(<JumpCard {...base} />);
    });

    it("should fall back to the default description", () => {
      expect(screen.getByText(/You're about to jump into Decentraland!/)).toBeTruthy();
    });
  });

  describe("when the location is a World", () => {
    beforeEach(() => {
      render(<JumpCard {...base} location={{ world: "foo.dcl.eth", parcel: "0, 0" }} />);
    });

    it("should label the World location", () => {
      expect(screen.getByText("World:", { exact: false }).parentElement?.textContent).toBe("World: foo.dcl.eth");
    });
  });

  describe("when actions are given", () => {
    beforeEach(() => {
      render(<JumpCard {...base} jumpInHref="/play/" actions={<button type="button">Custom</button>} />);
    });

    it("should render the actions", () => {
      expect(screen.getByRole("button", { name: "Custom" })).toBeTruthy();
    });

    it("should not render Jump In", () => {
      expect(screen.queryByRole("link", { name: /Jump In/ })).toBeNull();
    });
  });

  describe.each([
    [{ kind: "live", count: 4 } as const, "LIVE +4"],
    [{ kind: "live", count: 0 } as const, "LIVE +0"],
    [{ kind: "attendees", count: 9 } as const, "+9"],
    [{ kind: "players", count: 2 } as const, "2"],
  ])("when the badge is %o", (badge, text) => {
    beforeEach(() => {
      render(<JumpCard {...base} badge={badge} />);
    });

    it(`should show ${text}`, () => {
      expect(screen.getByText(text)).toBeTruthy();
    });
  });
});

describe("JumpEventActions", () => {
  describe("when the event is upcoming with a reminder set and a calendar link", () => {
    let onToggle: Mock;

    beforeEach(() => {
      onToggle = vi.fn();
      render(
        <JumpEventActions
          state="upcoming"
          calendarHref="https://calendar.google.com/x"
          remind={{ active: true, pending: false, onToggle }}
          onShare={vi.fn()}
        />,
      );
    });

    it("should expose Remind me as pressed", () => {
      expect(screen.getByRole("button", { name: /remind me/i }).getAttribute("aria-pressed")).toBe("true");
    });

    it("should open Add to calendar in a new tab", () => {
      expect(screen.getByRole("link", { name: "Add to calendar" }).getAttribute("target")).toBe("_blank");
    });

    describe("and Remind me is clicked", () => {
      beforeEach(async () => {
        await userEvent.setup().click(screen.getByRole("button", { name: /remind me/i }));
      });

      it("should toggle the reminder", () => {
        expect(onToggle).toHaveBeenCalledOnce();
      });
    });
  });

  describe("when the reminder is pending and there is no calendar link", () => {
    beforeEach(() => {
      render(<JumpEventActions state="upcoming" remind={{ active: false, pending: true, onToggle: vi.fn() }} onShare={vi.fn()} />);
    });

    it("should disable Remind me", () => {
      expect((screen.getByRole("button", { name: /remind me/i }) as HTMLButtonElement).disabled).toBe(true);
    });

    it("should omit Add to calendar", () => {
      expect(screen.queryByRole("link", { name: "Add to calendar" })).toBeNull();
    });
  });

  describe("when Share is clicked", () => {
    let onShare: Mock;

    beforeEach(async () => {
      onShare = vi.fn();
      render(
        <JumpEventActions
          state="upcoming"
          remind={{ active: false, pending: false, onToggle: vi.fn() }}
          onShare={onShare}
          shareNotice="Link copied to clipboard."
        />,
      );
      await userEvent.setup().click(screen.getByRole("button", { name: "Share" }));
    });

    it("should share", () => {
      expect(onShare).toHaveBeenCalledOnce();
    });

    it("should announce the notice", () => {
      expect(screen.getByRole("status").textContent).toBe("Link copied to clipboard.");
    });
  });

  describe("when the event has ended", () => {
    beforeEach(() => {
      render(<JumpEventActions state="ended" eventsHref="/events" />);
    });

    it("should point to the events list", () => {
      expect(screen.getByRole("link", { name: /explore events/i }).getAttribute("href")).toBe("/events");
    });
  });

  describe("when the event was deleted", () => {
    beforeEach(() => {
      render(<JumpEventActions state="deleted" eventsHref="/events" />);
    });

    it("should tell the event is gone", () => {
      expect(screen.getByText(/This event is no longer available/)).toBeTruthy();
    });

    it("should link to all events", () => {
      expect(screen.getByRole("link", { name: "All Events" }).getAttribute("href")).toBe("/events");
    });
  });
});

describe("JumpInvalid", () => {
  let hrefs: { jumpInHref: string; eventsHref: string; homeHref: string };

  beforeEach(() => {
    hrefs = { jumpInHref: "/play/", eventsHref: "/events", homeHref: "/" };
  });

  describe("when the link is a place link", () => {
    beforeEach(() => {
      render(<JumpInvalid kind="place" {...hrefs} />);
    });

    it("should not offer events", () => {
      expect(screen.queryByRole("link", { name: /explore events/i })).toBeNull();
    });

    it("should show the error heading", () => {
      expect(screen.getByRole("heading", { name: "Oops!" })).toBeTruthy();
    });

    it("should offer Jump In", () => {
      expect(screen.getByRole("link", { name: /Jump into Decentraland/ }).getAttribute("href")).toBe("/play/");
    });

    it("should offer the homepage", () => {
      expect(screen.getByRole("link", { name: /Go to Homepage/ }).getAttribute("href")).toBe("/");
    });
  });

  describe("when the link is an event link", () => {
    beforeEach(() => {
      render(<JumpInvalid kind="event" {...hrefs} />);
    });

    it("should offer events", () => {
      expect(screen.getByRole("link", { name: /explore events/i })).toBeTruthy();
    });
  });
});

describe("JumpPage", () => {
  describe("when WebGL is not available", () => {
    let getContext: MockInstance<HTMLCanvasElement["getContext"]>;
    let container: HTMLElement;

    beforeEach(() => {
      getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
      ({ container } = render(
        <JumpPage>
          <p>Card</p>
        </JumpPage>,
      ));
    });

    it("should keep the page content", () => {
      expect(screen.getByRole("main").textContent).toBe("Card");
    });

    it("should ask for a WebGL context", () => {
      expect(getContext).toHaveBeenCalledWith("webgl", { alpha: false, antialias: false });
    });

    it("should keep the still image and add no canvas", () => {
      expect(container.querySelector("canvas")).toBeNull();
    });
  });

  describe("when WebGL is available and StrictMode remounts the page", () => {
    let loseContext: Mock;
    let getContext: MockInstance<HTMLCanvasElement["getContext"]>;
    let raf: MockInstance<typeof window.requestAnimationFrame>;
    let container: HTMLElement;

    beforeEach(() => {
      loseContext = vi.fn();
      getContext = vi
        .spyOn(HTMLCanvasElement.prototype, "getContext")
        .mockImplementation(() => fakeContext(loseContext) as unknown as RenderingContext);
      raf = vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);
      ({ container } = render(
        <StrictMode>
          <JumpPage>
            <p>Card</p>
          </JumpPage>
        </StrictMode>,
      ));
    });

    it("should draw from a fresh canvas after the remount", () => {
      const asked = getContext.mock.contexts;
      expect(asked).toHaveLength(2);
      expect(asked[0]).not.toBe(asked[1]);
    });

    it("should release the first canvas's context", () => {
      expect(loseContext).toHaveBeenCalledOnce();
    });

    it("should leave exactly one canvas in the page, the fresh one", () => {
      expect(container.querySelectorAll("canvas")).toHaveLength(1);
      expect(container.querySelector("canvas")).toBe(getContext.mock.contexts[1]);
    });

    it("should start the animation", () => {
      expect(raf).toHaveBeenCalled();
    });
  });

  describe("when the visitor prefers reduced motion", () => {
    let matchMedia: Mock;
    let getContext: MockInstance<HTMLCanvasElement["getContext"]>;

    beforeEach(() => {
      matchMedia = vi.fn().mockReturnValue({ matches: true });
      Object.assign(window, { matchMedia });
      getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext");
      render(
        <JumpPage>
          <p>Card</p>
        </JumpPage>,
      );
    });

    afterEach(() => {
      Object.assign(window, { matchMedia: undefined });
    });

    it("should read the reduced motion preference", () => {
      expect(matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
    });

    it("should skip the animation", () => {
      expect(getContext).not.toHaveBeenCalled();
    });
  });
});
