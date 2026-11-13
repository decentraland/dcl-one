// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import CastNotFound from "./CastNotFound";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

beforeAll(() => {
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  })) as unknown as typeof window.matchMedia;
  globalThis.ResizeObserver ??= class {
    observe() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
afterEach(cleanup);

const trackCtx = { sid: "test", story: "landings/cast-not-found", variant: "instrumented" } as const;

function renderNotFound(onTrack: ReturnType<typeof vi.fn>) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={["/cast"]}>
        <CastNotFound trackCtx={trackCtx} from="watcher" reason="expired" onTrack={onTrack as never} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("when the Cast not-found page renders", () => {
  it.each([
    ["Go Home", "cast_not_found_go_home"],
    ["View Documentation", "cast_not_found_view_docs"],
  ])("should track a click on %s", async (name, event) => {
    const onTrack = vi.fn();
    renderNotFound(onTrack);
    const link = screen.getByRole("link", { name });
    link.addEventListener("click", (e) => e.preventDefault());
    await userEvent.click(link);
    expect(onTrack).toHaveBeenCalledWith(event, { from: "watcher", reason: "expired" }, trackCtx);
  });
});
