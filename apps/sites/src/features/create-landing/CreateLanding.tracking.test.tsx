// @vitest-environment jsdom
// jsdom has no layout, so the carousels' slides stay hidden: their controls are found by text.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import type { BlogPostSummary } from "@/data/lib/create-landing/blog-posts";
import type { HotScene } from "@/data/lib/create-landing/live-scenes";

import CreateLanding from "./CreateLanding";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

/** The document's readiness (its first page view), driven by the test. */
const readiness = vi.hoisted(() => ({ ready: true, pending: [] as Array<() => void> }));
function becomeReady() {
  readiness.ready = true;
  const queued = readiness.pending;
  readiness.pending = [];
  for (const fire of queued) fire();
}

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: (fire: () => void) => {
    if (readiness.ready) {
      fire();
      return () => {};
    }
    readiness.pending.push(fire);
    return () => {
      readiness.pending = readiness.pending.filter((queued) => queued !== fire);
    };
  },
}));

/** Every observed section, and a way to scroll a share of one into view. */
let observed: Array<{ target: Element; callback: IntersectionObserverCallback; observer: IntersectionObserver; threshold: unknown }>;
class FakeIntersectionObserver {
  constructor(
    private readonly callback: IntersectionObserverCallback,
    private readonly options: IntersectionObserverInit = {},
  ) {}
  observe(target: Element) {
    observed.push({ target, callback: this.callback, observer: this as unknown as IntersectionObserver, threshold: this.options.threshold });
  }
  disconnect() {
    observed = observed.filter((o) => o.observer !== (this as unknown as IntersectionObserver));
  }
  unobserve() {}
  takeRecords() {
    return [];
  }
}

function scrollInto(section: Element, ratio: number) {
  for (const { target, callback, observer } of observed.filter((o) => o.target === section)) {
    act(() => callback([{ target, isIntersecting: ratio > 0, intersectionRatio: ratio } as IntersectionObserverEntry], observer));
  }
}

/** The first slide's control with this text (each carousel also renders copies of its slides). */
const slideControl = (text: string) => screen.getAllByText(text)[0]!.closest("a, button") as HTMLElement;

const sectionOf = (name: string | RegExp) => screen.getByRole("heading", { level: 2, name }).closest("section")!;

const scene: HotScene = { id: "a", name: "Meta GamiMall", baseCoords: [1, 95], usersTotalCount: 6, parcels: [[1, 95]], thumbnail: "" };
const post: BlogPostSummary = {
  id: "p1",
  title: "Halloweeks 2026",
  publishedDate: "2026-10-05T01:00-07:00",
  url: "/blog/announcements/halloweeks-2026",
  image: null,
  category: { slug: "announcements", title: "Announcements", url: "/blog/announcements" },
};

function renderLanding({ scenes = [scene], posts = [post] }: { scenes?: HotScene[]; posts?: BlogPostSummary[] } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(["places", "hot-scenes"], scenes);
  client.setQueryData(["blog", "latest"], posts);
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <CreateLanding sid="sid-1" userId="0xabc" />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

/** Every event sent, with its properties less the audit timestamps. */
function sent() {
  return vi.mocked(track).mock.calls.map(([event, props, ctx]) => {
    const { track_called_at: _called, track_delivered_at: _delivered, track_deferred: _deferred, ...rest } = props as Record<string, unknown>;
    expect(ctx).toEqual({ sid: "sid-1", userId: "0xabc", legacy: true });
    return [event, rest];
  });
}

function blockNavigation(event: MouseEvent) {
  event.preventDefault();
}

let mobile: boolean;

beforeEach(() => {
  readiness.ready = true;
  readiness.pending = [];
  observed = [];
  mobile = false;
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: mobile && query === "(max-width: 767.95px)",
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    disconnect() {}
  });
  vi.spyOn(window, "scrollBy").mockImplementation(() => {});
  document.addEventListener("click", blockNavigation);
});
afterEach(() => {
  document.removeEventListener("click", blockNavigation);
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.mocked(track).mockReset();
});

describe("/create Click", () => {
  it("sends place Creators Hero for the mobile docs call and title scroll-to-why for the chevron", async () => {
    renderLanding();
    await userEvent.click(screen.getByRole("link", { name: "Explore the Creator Docs" }));
    await userEvent.click(screen.getByRole("button", { name: /scroll/i }));
    expect(sent()).toEqual([
      ["Click", { place: "Creators Hero" }],
      ["Click", { place: "Creators Hero", title: "scroll-to-why" }],
    ]);
  });

  it("names a Why card by its title", async () => {
    renderLanding();
    await userEvent.click(within(sectionOf(/creators choose Decentraland/)).getAllByRole("link")[0]!);
    expect(sent()).toEqual([["Click", { place: "Creators Why", title: "Join a Community of Creators" }]]);
  });

  it("sends the card and tab for a tab, and the card, tab and link label for a useful link", async () => {
    renderLanding();
    await userEvent.click(slideControl("Smart Wearables"));
    await userEvent.click(slideControl("SDK 7 Docs"));
    expect(sent()).toEqual([
      ["Click", { place: "Creators Create", card: "design-unique-wearables", tab: "Smart Wearables" }],
      ["Click", { place: "Creators Create", card: "design-unique-wearables", tab: "Smart Wearables", title: "SDK 7 Docs" }],
    ]);
  });

  it("sends nothing when the arrow keys move between tabs", async () => {
    renderLanding();
    slideControl("Regular").focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(track).not.toHaveBeenCalled();
  });

  it("names a live scene by its name and the view-all link view-all", async () => {
    renderLanding();
    await userEvent.click(screen.getByRole("link", { name: /Meta GamiMall/ }));
    await userEvent.click(within(sectionOf(/what creators are building/)).getByRole("link", { name: /See all live scenes/ }));
    expect(sent()).toEqual([
      ["Click", { place: "Creators Live Scenes", title: "Meta GamiMall" }],
      ["Click", { place: "Creators Live Scenes", title: "view-all" }],
    ]);
  });

  it("names a testimonial by its author and the Discord call join-discord", async () => {
    renderLanding();
    const connect = sectionOf(/Decentraland creators say/);
    await userEvent.click(slideControl("MrDhingia"));
    await userEvent.click(within(connect).getByRole("link", { name: /Discord/ }));
    expect(sent()).toEqual([
      ["Click", { place: "Creators Connect", title: "MrDhingia" }],
      ["Click", { place: "Creators Connect", title: "join-discord" }],
    ]);
  });

  it("names a tutorial by its title and the two calls watch-more and submit-tutorial", async () => {
    renderLanding();
    const learn = sectionOf(/from Community Tutorials/);
    const links = within(learn).getAllByRole("link");
    await userEvent.click(links[0]!);
    await userEvent.click(links.at(-2)!);
    await userEvent.click(links.at(-1)!);
    expect(sent()).toEqual([
      ["Click", { place: "Creators Learn", title: "Blender for Beginners | Making Your First Hat" }],
      ["Click", { place: "Creators Learn", title: "watch-more" }],
      ["Click", { place: "Creators Learn", title: "submit-tutorial" }],
    ]);
  });

  it("names a blog post by its title and the view-all link view-all", async () => {
    renderLanding();
    await userEvent.click(screen.getByRole("link", { name: /Halloweeks 2026/ }));
    await userEvent.click(within(sectionOf(/from the blog/)).getByRole("link", { name: /Read the blog/ }));
    expect(sent()).toEqual([
      ["Click", { place: "Creators Blog", title: "Halloweeks 2026" }],
      ["Click", { place: "Creators Blog", title: "view-all" }],
    ]);
  });

  it("sends the question when a FAQ opens, nothing when it closes, and faqs-cta for the call", async () => {
    renderLanding();
    const question = screen.getByRole("button", { name: "What is Decentraland?" });
    await userEvent.click(question);
    await userEvent.click(question);
    const faqs = screen.getByRole("region", { name: "Frequently Asked Questions" });
    const links = within(faqs).getAllByRole("link");
    await userEvent.click(links.at(-1)!);
    expect(sent()).toEqual([
      ["Click", { place: "Creators Faqs", title: "What is Decentraland?" }],
      ["Click", { place: "Creators Faqs", title: "faqs-cta" }],
    ]);
  });
});

describe("/create Section Viewed", () => {
  const SECTIONS: Array<[RegExp, string]> = [
    [/creators choose Decentraland/, "Creators Why"],
    [/possibilities/, "Creators Create"],
    [/what creators are building/, "Creators Live Scenes"],
    [/Decentraland creators say/, "Creators Connect"],
    [/from Community Tutorials/, "Creators Learn"],
    [/from the blog/, "Creators Blog"],
    [/Frequently Asked/, "Creators Faqs"],
  ];

  it("watches each of the seven sections at a tenth of its area and sends nothing before one scrolls in", () => {
    renderLanding();
    expect(observed.map((o) => o.threshold)).toEqual(Array(7).fill(0.1));
    expect(track).not.toHaveBeenCalled();
  });

  it.each(SECTIONS)("sends section_viewed once for %s, the first time it scrolls in", (heading, section) => {
    renderLanding();
    const target = sectionOf(heading);
    scrollInto(target, 0.5);
    scrollInto(target, 1);
    expect(sent()).toEqual([["Section Viewed", { section_viewed: section, mobile: false }]]);
  });

  it("sends nothing while less than a tenth of the section is in view", () => {
    renderLanding();
    scrollInto(sectionOf(/creators choose Decentraland/), 0.05);
    expect(track).not.toHaveBeenCalled();
  });

  it("flags a phone-width viewport as mobile", () => {
    mobile = true;
    renderLanding();
    scrollInto(sectionOf(/Frequently Asked/), 0.2);
    expect(sent()).toEqual([["Section Viewed", { section_viewed: "Creators Faqs", mobile: true }]]);
  });

  it("watches no Live Scenes or Blog section when there is nothing to show", () => {
    renderLanding({ scenes: [], posts: [] });
    expect(observed).toHaveLength(5);
  });

  it("sends nothing where the browser has no IntersectionObserver", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    expect(() => renderLanding()).not.toThrow();
    expect(track).not.toHaveBeenCalled();
  });
});

describe("/create events made before the document is ready", () => {
  it("are dropped when the landing unmounts first: no stale Click or Section Viewed after readiness", async () => {
    readiness.ready = false;
    const { unmount } = renderLanding();
    await userEvent.click(screen.getByRole("link", { name: "Explore the Creator Docs" }));
    scrollInto(sectionOf(/creators choose Decentraland/), 0.5);
    unmount();
    becomeReady();
    expect(track).not.toHaveBeenCalled();
  });

  it("are sent once, deferred, when the landing is still mounted at readiness", async () => {
    readiness.ready = false;
    renderLanding();
    await userEvent.click(screen.getByRole("link", { name: "Explore the Creator Docs" }));
    scrollInto(sectionOf(/creators choose Decentraland/), 0.5);
    becomeReady();
    expect(sent()).toEqual([
      ["Click", { place: "Creators Hero" }],
      ["Section Viewed", { section_viewed: "Creators Why", mobile: false }],
    ]);
    expect(vi.mocked(track).mock.calls.map(([, props]) => (props as { track_deferred: boolean }).track_deferred)).toEqual([true, true]);
  });
});
