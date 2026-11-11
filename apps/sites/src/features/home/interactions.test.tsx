// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Carousel } from "@dcl/one-ui/apps/sites/shared/Carousel";
import { CatchTheVibe } from "@dcl/one-ui/apps/sites/home/CatchTheVibe";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
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
const labels = {
  previous: "Previous",
  next: "Next",
  goTo: (n: number) => `Slide ${n}`,
  position: (n: number, total: number) => `${n} of ${total}`,
};
const clips = [
  {
    id: "a",
    image: "/poster.webp",
    video: "/clip.mp4",
    creator: { name: "A" },
  },
];
let play: ReturnType<typeof vi.spyOn>;
let pause: ReturnType<typeof vi.spyOn>;
let wide: boolean;
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

beforeEach(() => {
  wide = true;
  mediaListeners = new Map();
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({
      get matches() {
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
  it("clears a short drag released outside before capture and ignores later buttonless movement", () => {
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
  it("cancels a drag on lost capture and pointer cancellation without changing slides", () => {
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
  it("preserves normal anchor clicks and vertical touch while suppressing native drag", () => {
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
  it("wraps with controls and keeps the active slide keyboard reachable", () => {
    renderCarousel();
    fireEvent.click(screen.getByRole("button", { name: "Previous" }));
    expect(
      screen
        .getByRole("group", { name: "2 of 2" })
        .querySelector("a")
        ?.getAttribute("href"),
    ).toBe("/two");
    fireEvent.keyDown(screen.getByRole("group", { name: "2 of 2" }), {
      key: "ArrowRight",
    });
    expect(screen.getByRole("group", { name: "1 of 2" })).toBeTruthy();
  });
});

describe("carousel autoplay", () => {
  it("pauses on hover and drag, then resumes after cancellation", () => {
    vi.useFakeTimers();
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

describe("clip playback intent", () => {
  it("does not restart a pending hover play after leave aborts it", async () => {
    let reject!: (reason: unknown) => void;
    play.mockReturnValueOnce(
      new Promise<void>((_, fail) => {
        reject = fail;
      }),
    );
    const { container } = renderVibe();
    const card = container.querySelector("ul > li")!;
    fireEvent.mouseEnter(card);
    fireEvent.mouseLeave(card);
    await act(async () => {
      reject(new DOMException("paused", "AbortError"));
    });
    expect(play).toHaveBeenCalledOnce();
    expect(pause).toHaveBeenCalled();
    expect(
      card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
    ).toBe("false");
  });
  it("retries only the current audible attempt when sound is denied", async () => {
    play.mockRejectedValueOnce(
      new DOMException("sound denied", "NotAllowedError"),
    );
    const { container } = renderVibe();
    const card = container.querySelector("ul > li")!;
    await act(async () => fireEvent.click(card.querySelector("button")!));
    expect(play).toHaveBeenCalledTimes(2);
    expect(card.querySelector("video")?.muted).toBe(true);
    expect(
      card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
    ).toBe("true");
  });
  it("stops the current attempt when its muted retry also fails", async () => {
    play
      .mockRejectedValueOnce(
        new DOMException("sound denied", "NotAllowedError"),
      )
      .mockRejectedValueOnce(new Error("codec failed"));
    const { container } = renderVibe();
    const card = container.querySelector("ul > li")!;
    await act(async () => fireEvent.click(card.querySelector("button")!));
    expect(play).toHaveBeenCalledTimes(2);
    expect(
      card.querySelector("[aria-pressed]")?.getAttribute("aria-pressed"),
    ).toBe("false");
  });
  it("does not let an obsolete rejection restart or stop a newer attempt", async () => {
    let reject!: (reason: unknown) => void;
    play.mockReturnValueOnce(
      new Promise<void>((_, fail) => {
        reject = fail;
      }),
    );
    const { container } = renderVibe();
    const button = container.querySelector("ul > li button")!;
    fireEvent.click(button);
    fireEvent.click(button);
    fireEvent.click(button);
    await act(async () => {
      reject(new DOMException("old denial", "NotAllowedError"));
    });
    expect(play).toHaveBeenCalledTimes(2);
    expect(button.getAttribute("aria-pressed")).toBe("true");
  });
  it("never retries an aborted audible attempt or an unsupported muted clip", async () => {
    play.mockRejectedValueOnce(new DOMException("abort", "AbortError"));
    const { container, unmount } = renderVibe();
    await act(async () =>
      fireEvent.click(container.querySelector("ul > li button")!),
    );
    expect(play).toHaveBeenCalledOnce();
    unmount();
    play.mockClear();
    play.mockRejectedValueOnce(new Error("unsupported"));
    const second = renderVibe();
    await act(async () =>
      fireEvent.mouseEnter(second.container.querySelector("ul > li")!),
    );
    expect(play).toHaveBeenCalledOnce();
  });
  it("invalidates pending playback on unmount", async () => {
    let reject!: (reason: unknown) => void;
    play.mockReturnValueOnce(
      new Promise<void>((_, fail) => {
        reject = fail;
      }),
    );
    const { container, unmount } = renderVibe();
    fireEvent.click(container.querySelector("ul > li button")!);
    pause.mockClear();
    unmount();
    expect(pause).toHaveBeenCalled();
    await act(async () => {
      reject(new DOMException("late denial", "NotAllowedError"));
    });
    expect(play).toHaveBeenCalledOnce();
  });
  it("loads metadata only for the visible layout and active carousel slide", () => {
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
  it("renders the mobile store branch without a desktop installer action", () => {
    const { store } = downloadActions(
      { kind: "mobile", os: "android" },
      "+438K",
    );
    render(<ComeHangOut title="Come" store={store} />);
    const anchor = screen.getByRole("link");
    expect(anchor.getAttribute("href")).toContain("play.google.com");
    expect(anchor.getAttribute("aria-label")).toBe("Get it on Google Play");
    expect(document.querySelector('a[href$=".exe"]')).toBeNull();
  });
});
