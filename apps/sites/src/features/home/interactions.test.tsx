// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Carousel, type CarouselLabels, CatchTheVibe, ComeHangOut } from "@dcl/one-ui";
import { downloadActions } from "../download/actions";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$:
      () =>
      <T,>(type: T) =>
        type,
  }),
);
let labels: CarouselLabels;
let clips: Parameters<typeof CatchTheVibe>[0]["items"];
let play: ReturnType<typeof vi.spyOn>;
let pause: ReturnType<typeof vi.spyOn>;
let wide: boolean;
let reduced: boolean;
let mediaListeners: Map<string, () => void>;
function renderCarousel(autoplayDelay = 0) {
  return render(
    <>
      <h2 id="rituals">Rituals</h2>
      <Carousel
        labelledBy="rituals"
        autoplayDelay={autoplayDelay}
        items={["one", "two"]}
        getKey={(s) => s}
        labels={labels}
        renderSlide={(s) => (
          <a href={`/${s}`}>
            <img src="/slide.webp" alt={s} />
          </a>
        )}
      />
    </>,
  );
}
function renderVibe() {
  return render(
    <CatchTheVibe
      title="Clips"
      items={clips}
      labels={{ play: "Play", pause: "Pause", mute: "Mute", unmute: "Unmute" }}
      carouselLabels={labels}
    />,
  );
}
/** A play() that stays pending until the test rejects it. */
function pendingPlay() {
  let reject!: (reason: unknown) => void;
  play.mockReturnValueOnce(
    new Promise<void>((_, fail) => {
      reject = fail;
    }),
  );
  return (reason: unknown) => reject(reason);
}

beforeEach(() => {
  labels = {
    previous: "Previous",
    next: "Next",
    goTo: (n: number) => `Slide ${n}`,
    position: (n: number, total: number) => `${n} of ${total}`,
  };
  clips = [
    {
      id: "a",
      image: "/poster.webp",
      video: "/clip.mp4",
      creator: { name: "A" },
    },
  ];
  wide = true;
  reduced = false;
  mediaListeners = new Map();
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({
      get matches() {
        if (query === "(prefers-reduced-motion: reduce)") return reduced;
        return query.includes("min-width")
          ? wide
          : query.includes("no-preference");
      },
      addEventListener: (_: string, callback: () => void) =>
        mediaListeners.set(query, callback),
      removeEventListener: vi.fn(),
    })),
  );
  vi.stubGlobal(
    "PointerEvent",
    class extends MouseEvent {
      pointerId: number;
      pointerType: string;
      constructor(type: string, init: PointerEventInit = {}) {
        super(type, init);
        this.pointerId = init.pointerId ?? 1;
        this.pointerType = init.pointerType ?? "mouse";
      }
    },
  );
  play = vi
    .spyOn(HTMLMediaElement.prototype, "play")
    .mockResolvedValue(undefined);
  pause = vi
    .spyOn(HTMLMediaElement.prototype, "pause")
    .mockImplementation(() => {});
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("carousel pointer lifecycle", () => {
  describe("when the visitor prefers reduced motion", () => {
    beforeEach(() => {
      reduced = true;
    });

    describe.each(["Previous", "Next"])("and %s wraps around", (direction) => {
      it("should keep the track on the canonical copy", () => {
        const { container } = renderCarousel();
        fireEvent.click(screen.getByRole("button", { name: direction }));

        const track = container.querySelector("[data-animated]")!;
        expect(track.getAttribute("style")).toContain("--carousel-position: 3");
      });
    });
  });

  describe("when a drag starts during a transition", () => {
    describe("and the transition is an in-flight clone", () => {
      it("should settle it before the drag interrupts the transition", () => {
        const { container } = renderCarousel();
        const carousel = container.querySelector(
          '[aria-roledescription="carousel"]',
        )!;
        fireEvent.click(screen.getByRole("button", { name: "Previous" }));
        expect(container.querySelector('[data-animated="true"]')).toBeTruthy();

        fireEvent.pointerDown(carousel, {
          button: 0,
          pointerId: 1,
          pointerType: "mouse",
          clientX: 100,
          clientY: 0,
        });

        const track = container.querySelector("[data-animated]")!;
        expect(track.getAttribute("data-animated")).toBe("false");
        expect(track.getAttribute("style")).toContain("--carousel-position: 3");
      });
    });

    describe("and the transition is in range", () => {
      it("should disable the transition before the drag starts", () => {
        const { container } = renderCarousel();
        const carousel = container.querySelector(
          '[aria-roledescription="carousel"]',
        )!;
        fireEvent.click(screen.getByRole("button", { name: "Next" }));
        expect(container.querySelector('[data-animated="true"]')).toBeTruthy();

        fireEvent.pointerDown(carousel, {
          button: 0,
          pointerId: 1,
          pointerType: "mouse",
          clientX: 100,
          clientY: 0,
        });

        expect(container.querySelector('[data-animated="false"]')).toBeTruthy();
        expect(
          container.querySelector("[data-animated]")?.getAttribute("style"),
        ).toContain("--carousel-position: 3");
      });
    });
  });

  describe("when a short drag is released outside before capture", () => {
    it("should clear the drag and ignore later buttonless movement", () => {
      const { container } = renderCarousel();
      const carousel = container.querySelector(
        '[aria-roledescription="carousel"]',
      )!;
      fireEvent.pointerDown(carousel, {
        button: 0,
        buttons: 1,
        clientX: 100,
        clientY: 100,
      });
      fireEvent.pointerMove(carousel, { buttons: 1, clientX: 120, clientY: 100 });
      expect(carousel.getAttribute("data-dragging")).toBe("true");
      fireEvent.pointerLeave(carousel, {
        buttons: 1,
        clientX: 130,
        clientY: 200,
      });
      fireEvent.pointerUp(window, { buttons: 0, clientX: 130, clientY: 200 });
      fireEvent.pointerMove(carousel, { buttons: 0, clientX: 300, clientY: 100 });
      expect(carousel.getAttribute("data-dragging")).toBe("false");
      expect(container.querySelector("[style]")?.getAttribute("style")).toContain(
        "--carousel-drag: 0px",
      );
      expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
    });
  });

  describe("when a drag loses capture or the pointer is cancelled", () => {
    it("should cancel the drag without changing slides", () => {
      const { container, unmount } = renderCarousel();
      const carousel = container.querySelector(
        '[aria-roledescription="carousel"]',
      )!;
      for (const end of ["lostPointerCapture", "pointerCancel"] as const) {
        fireEvent.pointerDown(carousel, {
          button: 0,
          buttons: 1,
          clientX: 100,
          clientY: 100,
        });
        fireEvent.pointerMove(carousel, {
          buttons: 1,
          clientX: 120,
          clientY: 100,
        });
        fireEvent[end](carousel);
        expect(carousel.getAttribute("data-dragging")).toBe("false");
      }
      expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
      unmount();
      fireEvent.pointerUp(window, { buttons: 0 });
    });
  });

  describe("when a slide link is clicked or touched vertically", () => {
    it("should preserve normal anchor clicks and vertical touch while suppressing native drag", () => {
      const { container } = renderCarousel();
      const carousel = container.querySelector(
        '[aria-roledescription="carousel"]',
      )!;
      const anchor = screen.getByRole("link");
      const click = vi.fn((event: Event) => event.preventDefault());
      anchor.addEventListener("click", click);
      fireEvent.pointerDown(anchor, {
        button: 0,
        buttons: 1,
        clientX: 100,
        clientY: 100,
      });
      fireEvent.pointerUp(anchor, { buttons: 0, clientX: 100, clientY: 100 });
      fireEvent.click(anchor);
      expect(click).toHaveBeenCalledOnce();
      expect(fireEvent.dragStart(anchor)).toBe(false);
      fireEvent.pointerDown(carousel, {
        pointerType: "touch",
        button: 0,
        clientX: 100,
        clientY: 100,
      });
      fireEvent.pointerMove(carousel, {
        pointerType: "touch",
        clientX: 105,
        clientY: 220,
      });
      fireEvent.pointerUp(carousel, {
        pointerType: "touch",
        clientX: 105,
        clientY: 220,
      });
      expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
    });
  });

  describe("when the controls wrap around", () => {
    beforeEach(() => {
      renderCarousel();
      fireEvent.click(screen.getByRole("button", { name: "Previous" }));
    });

    it("should show the last slide", () => {
      expect(
        screen
          .getByRole("group", { name: "2 of 2" })
          .querySelector("a")
          ?.getAttribute("href"),
      ).toBe("/two");
    });

    it("should keep the active slide keyboard reachable", () => {
      fireEvent.keyDown(screen.getByRole("group", { name: "2 of 2" }), {
        key: "ArrowRight",
      });
      expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
    });
  });
});

describe("carousel autoplay", () => {
  describe("when the carousel autoplays and is hovered, dragged and cancelled", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    it("should pause on hover and drag, then resume after cancellation", () => {
      const { container } = renderCarousel(100);
      const carousel = container.querySelector(
        '[aria-roledescription="carousel"]',
      )!;
      act(() => vi.advanceTimersByTime(100));
      expect(screen.getByRole("group", { name: "2 of 2" })).toBeTruthy();
      fireEvent.pointerEnter(carousel, { pointerType: "mouse" });
      act(() => vi.advanceTimersByTime(500));
      expect(screen.getByRole("group", { name: "2 of 2" })).toBeTruthy();
      fireEvent.pointerLeave(carousel, { pointerType: "mouse" });
      fireEvent.pointerDown(carousel, {
        button: 0,
        buttons: 1,
        clientX: 100,
        clientY: 100,
      });
      act(() => vi.advanceTimersByTime(500));
      expect(screen.getByRole("group", { name: "2 of 2" })).toBeTruthy();
      fireEvent.pointerCancel(carousel);
      act(() => vi.advanceTimersByTime(100));
      expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
      fireEvent.pointerDown(screen.getByRole("button", { name: "Next" }), {
        button: 0,
        buttons: 1,
      });
      expect(carousel.getAttribute("data-dragging")).toBe("false");
    });
  });
});

describe("clip playback intent", () => {
  describe("when a pending hover play is aborted by leaving", () => {
    let rejectPlay: (reason: unknown) => void;

    beforeEach(() => {
      rejectPlay = pendingPlay();
    });

    it("should not restart it", async () => {
      const { container } = renderVibe();
      const card = container.querySelector("ul > li")!;
      fireEvent.mouseEnter(card);
      fireEvent.mouseLeave(card);
      await act(async () => {
        rejectPlay(new DOMException("paused", "AbortError"));
      });
      expect(play).toHaveBeenCalledOnce();
      expect(pause).toHaveBeenCalled();
      expect(
        card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
      ).toBe("false");
    });
  });

  describe("when sound is denied", () => {
    beforeEach(() => {
      play.mockRejectedValueOnce(
        new DOMException("sound denied", "NotAllowedError"),
      );
    });

    describe("and the muted retry succeeds", () => {
      it("should retry only the current audible attempt, muted", async () => {
        const { container } = renderVibe();
        const card = container.querySelector("ul > li")!;
        await act(async () => fireEvent.click(card.querySelector("button")!));
        expect(play).toHaveBeenCalledTimes(2);
        expect(card.querySelector("video")?.muted).toBe(true);
        expect(
          card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
        ).toBe("true");
      });
    });

    describe("and the muted retry also fails", () => {
      beforeEach(() => {
        play.mockRejectedValueOnce(new Error("codec failed"));
      });

      it("should stop the current attempt", async () => {
        const { container } = renderVibe();
        const card = container.querySelector("ul > li")!;
        await act(async () => fireEvent.click(card.querySelector("button")!));
        expect(play).toHaveBeenCalledTimes(2);
        expect(
          card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
        ).toBe("false");
      });
    });
  });

  describe("when an obsolete attempt is rejected after newer clicks", () => {
    let rejectPlay: (reason: unknown) => void;

    beforeEach(() => {
      rejectPlay = pendingPlay();
    });

    it("should not let the rejection restart or stop the newer attempt", async () => {
      const { container } = renderVibe();
      const button = container.querySelector("ul > li button")!;
      fireEvent.click(button);
      fireEvent.click(button);
      fireEvent.click(button);
      await act(async () => {
        rejectPlay(new DOMException("old denial", "NotAllowedError"));
      });
      expect(play).toHaveBeenCalledTimes(2);
      expect(button.getAttribute("aria-pressed")).toBe("true");
    });
  });

  describe("when an audible attempt is aborted", () => {
    beforeEach(() => {
      play.mockRejectedValueOnce(new DOMException("abort", "AbortError"));
    });

    it("should never retry it", async () => {
      const { container } = renderVibe();
      await act(async () =>
        fireEvent.click(container.querySelector("ul > li button")!),
      );
      expect(play).toHaveBeenCalledOnce();
    });
  });

  describe("when a muted clip is unsupported", () => {
    beforeEach(() => {
      play.mockRejectedValueOnce(new Error("unsupported"));
    });

    it("should never retry it", async () => {
      const { container } = renderVibe();
      await act(async () =>
        fireEvent.mouseEnter(container.querySelector("ul > li")!),
      );
      expect(play).toHaveBeenCalledOnce();
    });
  });

  describe("when the clips unmount with playback pending", () => {
    let rejectPlay: (reason: unknown) => void;

    beforeEach(() => {
      rejectPlay = pendingPlay();
    });

    it("should invalidate the pending playback", async () => {
      const { container, unmount } = renderVibe();
      fireEvent.click(container.querySelector("ul > li button")!);
      pause.mockClear();
      unmount();
      expect(pause).toHaveBeenCalled();
      await act(async () => {
        rejectPlay(new DOMException("late denial", "NotAllowedError"));
      });
      expect(play).toHaveBeenCalledOnce();
    });
  });

  describe("when the layout changes from wide to narrow", () => {
    it("should load metadata only for the visible layout and active carousel slide", () => {
      const { container } = renderVibe();
      expect(container.querySelectorAll("video[src]")).toHaveLength(1);
      act(() => {
        wide = false;
        mediaListeners.get("(min-width: 1024px)")?.();
      });
      expect(container.querySelectorAll("video[src]")).toHaveLength(1);
      for (const video of container.querySelectorAll("video:not([src])"))
        expect(video.getAttribute("preload")).toBe("none");
    });
  });
});

describe("ComeHangOut", () => {
  describe("when the visitor is on an Android phone", () => {
    let anchor: HTMLElement;

    beforeEach(() => {
      const { store } = downloadActions(
        { kind: "mobile", os: "android" },
        "+438K",
      );
      render(<ComeHangOut title="Come" store={store} />);
      anchor = screen.getByRole("link");
    });

    it("should link to the Play Store", () => {
      expect(anchor.getAttribute("href")).toContain("play.google.com");
      expect(anchor.getAttribute("aria-label")).toBe("Get it on Google Play");
    });

    it("should not offer a desktop installer action", () => {
      expect(document.querySelector('a[href$=".exe"]')).toBeNull();
    });
  });
});
