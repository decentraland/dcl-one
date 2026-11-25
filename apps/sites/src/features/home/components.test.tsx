// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Hero, ComeHangOut, WhatsOn, Carousel, type CarouselLabels } from "@dcl/one-ui";
import { downloadActions } from "../download/actions";
import { homeDownloadActions } from "./actions";

vi.hoisted(() =>
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true }),
);

describe("home sections", () => {
  let labels: CarouselLabels;

  beforeEach(() => {
    labels = {
      previous: "Previous",
      next: "Next",
      goTo: (n: number) => `Slide ${n}`,
      position: (n: number, total: number) => `${n} of ${total}`,
    };
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.resetAllMocks();
  });

  describe("when the download falls back to Windows", () => {
    it.each([
      [
        "a Linux desktop without an installer host",
        { kind: "desktop", os: "other", arch: "unknown" },
        false,
      ],
      [
        "32-bit Windows with an installer host",
        { kind: "desktop", os: "windows", arch: "ia32" },
        true,
      ],
    ] as const)(
      "names the fallback call's Windows icon for %s",
      (_, platform, installers) => {
        const { hero, hangOut, store } = homeDownloadActions(
          platform,
          "+438K",
          installers,
        );
        render(
          <>
            <Hero
              title="Come Hang Out"
              imageAlt="Decentraland"
              actions={hero}
              mobile={{ title: "Anywhere", subtitle: "Mobile", store }}
            />
            <ComeHangOut title="Come Hang Out" actions={hangOut} store={store} />
          </>,
        );
        const calls = screen.getAllByRole("link", {
          name: "DOWNLOAD FOR Windows",
        });
        expect(calls).toHaveLength(2);
        for (const call of calls) {
          expect(call.getAttribute("href")).toBe("/download");
          expect(within(call).getByRole("img", { name: "Windows" })).toBeTruthy();
        }
      },
    );
  });

  describe("when the hero and the closing section render with a download count", () => {
    let html: string;

    beforeEach(() => {
      const { actions, store } = downloadActions({ kind: "unknown" }, "+438K");
      html = renderToString(
        <>
          <Hero
            title="Come Hang Out"
            imageAlt="Decentraland"
            actions={actions}
            mobile={{ title: "Anywhere", subtitle: "Mobile", store }}
          />
          <ComeHangOut title="Come Hang Out" actions={actions} store={store} />
        </>,
      );
    });

    it("should render both counters", () => {
      expect(html.match(/Total Downloads: \+438K/g)).toHaveLength(2);
    });

    it("should render the single solid white product badge", () => {
      expect(html.match(/viewBox="0 0 18 18"/g)).toHaveLength(2);
      expect(html).not.toContain("linearGradient");
    });

    it("should link the download to the success page instead of the installer", () => {
      expect(html).toContain(
        'href="/download_success?os=windows&amp;arch=amd64"',
      );
      expect(html).not.toContain("Decentraland_installer.exe");
    });

    it("should offer the App Store", () => {
      expect(html).toContain("Download on the App Store");
    });
  });

  describe("when What's On has no event cards from the backend", () => {
    it("should render nothing", () => {
      expect(
        renderToString(
          <WhatsOn
            title="Live"
            cards={[]}
            labels={{ live: "LIVE", by: "by", jumpIn: "Jump in" }}
            carouselLabels={labels}
            viewAll={{ label: "All", href: "/events" }}
          />,
        ),
      ).toBe("");
    });
  });

  describe("when an event card has no player count", () => {
    let html: string;

    beforeEach(() => {
      html = renderToString(
        <WhatsOn
          title="Live"
          cards={[
            {
              id: "event",
              title: "Real event",
              image: null,
              href: "/events?id=event",
            },
          ]}
          labels={{ live: "LIVE", by: "by", jumpIn: "Jump in" }}
          carouselLabels={labels}
          viewAll={{ label: "All", href: "/events" }}
        />,
      );
    });

    it("should render the event", () => {
      expect(html).toContain("Real event");
    });

    it("should not invent a player count", () => {
      expect(html).not.toContain(">0<");
    });
  });

  describe("when the carousel's next control is clicked", () => {
    beforeEach(() => {
      render(
        <>
          <h2 id="rituals">Rituals</h2>
          <Carousel
            labelledBy="rituals"
            items={["one", "two", "three"]}
            getKey={(s) => s}
            labels={labels}
            renderSlide={(s) => <a href={`/${s}`}>{s}</a>}
          />
        </>,
      );
      fireEvent.click(screen.getByRole("button", { name: "Next" }));
    });

    it("should move to the next slide", () => {
      expect(screen.getByRole("group", { name: "2 of 3" })).toBeTruthy();
    });

    it("should keep inactive links inert", () => {
      expect(document.querySelectorAll("[inert]").length).toBeGreaterThan(0);
    });
  });

  describe("when the carousel is empty", () => {
    let html: string;

    beforeEach(() => {
      html = renderToString(
        <Carousel
          labelledBy="empty"
          items={[]}
          getKey={(s) => String(s)}
          labels={labels}
          renderSlide={() => null}
        />,
      );
    });

    it("should not emit invalid arithmetic", () => {
      expect(html).not.toContain("NaN");
      expect(html).not.toContain("Infinity");
    });
  });
});

describe("the home hero's ambient video", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.resetAllMocks();
  });

  // A window of `width` px: every min-width query up to it matches.
  const at = (width: number, reduced = false) => {
    vi.stubGlobal("matchMedia", (query: string) => {
      const min = /min-width:\s*(\d+)px/.exec(query);
      const matches = query.includes("reduce") ? reduced : min ? width >= Number(min[1]) : false;
      return { matches, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() };
    });
    vi.stubGlobal("requestIdleCallback", (start: () => void) => {
      start();
      return 1;
    });
    vi.stubGlobal("cancelIdleCallback", vi.fn());
  };
  const video = () => {
    const { container } = render(<Hero title="Come Hang Out" imageAlt="Decentraland" />);
    return container.querySelector("video");
  };

  describe("when the window is at the foundations' 1024px desktop breakpoint", () => {
    beforeEach(() => at(1024));

    it("should play", () => {
      expect(video()).not.toBeNull();
    });
  });

  describe("when the window is 1000px wide, where the wide layout already applies", () => {
    beforeEach(() => at(1000));

    it("should not load", () => {
      expect(video()).toBeNull();
    });
  });

  describe("when the visitor prefers reduced motion", () => {
    beforeEach(() => at(1440, true));

    it("should not load", () => {
      expect(video()).toBeNull();
    });
  });

});
