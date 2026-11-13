// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { afterEach, beforeAll, expect, it, vi } from "vitest";

import type { BlogPostSummary } from "@/data/lib/create-landing/blog-posts";
import type { HotScene } from "@/data/lib/create-landing/live-scenes";

import CreateLanding from "./CreateLanding";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

beforeAll(() => {
  window.matchMedia ??= ((query: string) => ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {} })) as unknown as typeof window.matchMedia;
  globalThis.ResizeObserver ??= class {
    observe() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
afterEach(cleanup);

const scene = (id: string, name: string, users: number): HotScene => ({
  id,
  name,
  baseCoords: [1, 95],
  usersTotalCount: users,
  parcels: [[1, 95]],
  thumbnail: "",
});

const post: BlogPostSummary = {
  id: "p1",
  title: "Halloweeks 2026",
  publishedDate: "2026-10-05T01:00-07:00",
  url: "/blog/announcements/halloweeks-2026",
  image: { url: "/blog/_media/760gckxgmBstLrWqgvDAM2/blog-hero.webp", width: 768, height: 384 },
  category: { slug: "announcements", title: "Announcements", url: "/blog/announcements" },
};

function landing(scenes: HotScene[], posts: BlogPostSummary[] = [post]) {
  const client = new QueryClient();
  client.setQueryData(["public", "hot-scenes"], scenes);
  client.setQueryData(["public", "blog", "latest"], posts);
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <CreateLanding />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

it("renders on the server with the hero title and every section", () => {
  const html = renderToString(landing([scene("a", "Meta GamiMall", 6)]));
  expect(html).toContain("Create your own");
  for (const title of ["creators choose Decentraland", "possibilities", "what creators are building right now", "Decentraland creators say", "from Community Tutorials", "from the blog", "Frequently Asked"]) {
    expect(html).toContain(title);
  }
});

it("lists the live scenes with their visitors and hides the section with none", () => {
  render(landing([scene("a", "Meta GamiMall", 6), scene("b", "Genesis Plaza", 50)]));
  const link = screen.getByRole("link", { name: /Meta GamiMall/ });
  expect(link.getAttribute("href")).toBe("/places/place/1,95");
  expect(within(link).getByText("6 online")).toBeTruthy();
  expect(screen.queryByText("Genesis Plaza")).toBeNull();
  cleanup();
  render(landing([]));
  expect(screen.queryByText("See all live scenes")).toBeNull();
});

it("shows the newest posts with their picture, category and date, and hides the section with none", () => {
  render(landing([]));
  const link = screen.getByRole("link", { name: /Halloweeks 2026/ });
  expect(link.getAttribute("href")).toBe("/blog/announcements/halloweeks-2026");
  expect(within(link).getByRole("img").getAttribute("src")).toBe("/blog/_media/760gckxgmBstLrWqgvDAM2/blog-hero.webp");
  expect(within(link).getByText("Announcements")).toBeTruthy();
  expect(within(link).getByText("October 5, 2026")).toBeTruthy();
  cleanup();
  render(landing([], []));
  expect(screen.queryByText("Read the blog")).toBeNull();
});

it("opens one FAQ answer at a time", () => {
  render(landing([]));
  const first = screen.getByRole("button", { name: "What is Decentraland?" });
  const second = screen.getByRole("button", { name: "What can I create in Decentraland?" });
  fireEvent.click(first);
  expect(first.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(second);
  expect(first.getAttribute("aria-expanded")).toBe("false");
  expect(second.getAttribute("aria-expanded")).toBe("true");
});
