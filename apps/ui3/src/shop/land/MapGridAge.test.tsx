import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

import MapGridAge from "./MapGridAge";

const MINUTE = 60_000;

/** Answers the map API's tiles info with `answer`, and records the reads. */
function serveTilesInfo(answer: (signal?: AbortSignal) => Promise<Response>) {
  const reads: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      reads.push(url);
      return answer(init?.signal ?? undefined);
    }),
  );
  return reads;
}

/** A read that never answers, rejecting when aborted as `fetch` does. */
const hanging = (signal?: AbortSignal) =>
  new Promise<Response>((_, reject) => signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError"))));

/** The map API's own answer while it serves no grid. */
const unavailable = () =>
  Response.json({ error: "Service Unavailable", message: "the tile grid has not been built yet", source: "map-grid" }, { status: 503 });

function renderAge() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MapGridAge />
    </QueryClientProvider>,
  );
}

/** The server's clock now. */
const serverNow = () => Date.now();
/** The map API's answer as a cross-origin read sees it: its clock in the body (`serverNow`), no readable `Date`. */
const fromServer = (body: object, at: number) => Response.json({ ...body, serverNow: at });

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("MapGridAge", () => {
  it("says how long ago the map API last found its grid current, with a decorative clock", async () => {
    const reads = serveTilesInfo(async () => Response.json({ lastUpdatedAt: Date.now() - 600 * MINUTE, checkedAt: Date.now() - 12 * MINUTE }));
    renderAge();
    const label = await screen.findByText("Updated 12 min ago");
    const mark = label.querySelector("svg");
    expect(mark).not.toBeNull();
    expect(mark).toHaveAttribute("aria-hidden", "true");
    expect(reads).toEqual(["/v2/tiles/info"]);
  });

  it.each([
    ["a grid found current in the last five minutes", () => Response.json({ checkedAt: Date.now() - 4 * MINUTE })],
    ["an answer without checkedAt", () => Response.json({ lastUpdatedAt: Date.now() - 600 * MINUTE })],
    ["a map API that cannot serve its grid", unavailable],
    ["a failed read", () => Promise.reject(new TypeError("Failed to fetch"))],
  ])("shows nothing for %s", async (_case, answer) => {
    const reads = serveTilesInfo(async () => answer());
    const { container } = renderAge();
    await waitFor(() => expect(reads).toHaveLength(1));
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });

  it("measures the age on the server's clock, whether this device runs ahead or behind", async () => {
    const server = serverNow();
    // Six minutes ahead, on a grid the server checked a minute ago: no label.
    vi.useFakeTimers({ now: server + 6 * MINUTE, shouldAdvanceTime: true });
    serveTilesInfo(async () => fromServer({ checkedAt: server - MINUTE }, server));
    const ahead = renderAge();
    await waitFor(() => expect(fetch).toHaveBeenCalledTimes(1));
    await act(async () => {});
    expect(ahead.container).toBeEmptyDOMElement();
    ahead.unmount();

    // Ten minutes behind, on a grid the server checked twelve minutes ago.
    vi.setSystemTime(server - 10 * MINUTE);
    serveTilesInfo(async () => fromServer({ checkedAt: server - 12 * MINUTE }, server));
    renderAge();
    expect(await screen.findByText("Updated 12 min ago")).toBeInTheDocument();
  });

  it("keeps the last answer through a gateway's error, which says nothing about the grid", async () => {
    const server = serverNow();
    vi.useFakeTimers({ now: server, shouldAdvanceTime: true });
    let first = true;
    serveTilesInfo(async () => {
      if (!first) return new Response(null, { status: 502 });
      first = false;
      return fromServer({ checkedAt: server - 30 * MINUTE }, server);
    });
    renderAge();
    expect(await screen.findByText("Updated 30 min ago")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2 * MINUTE);
    });
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(screen.getByText("Updated 32 min ago")).toBeInTheDocument();
  });

  it("gives up a read that never settles within a minute, so polling goes on", async () => {
    const server = serverNow();
    vi.useFakeTimers({ now: server, shouldAdvanceTime: true });
    const answers = [
      async () => fromServer({ checkedAt: server - 30 * MINUTE }, server),
      hanging,
      async () => fromServer({ checkedAt: server + MINUTE }, server + 2 * MINUTE),
    ];
    let read = 0;
    serveTilesInfo((signal) => answers[Math.min(read++, answers.length - 1)]!(signal));
    renderAge();
    expect(await screen.findByText("Updated 30 min ago")).toBeInTheDocument();
    // The second read hangs and gives up within the minute; the poll after it (a minute after that
    // failure, as TanStack counts) reads again and finds the grid current.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3 * MINUTE);
    });
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(screen.queryByText(/Updated/)).toBeNull();
  });

  it("keeps ageing the last answer while later reads never settle", async () => {
    const server = serverNow();
    vi.useFakeTimers({ now: server, shouldAdvanceTime: true });
    let first = true;
    serveTilesInfo(() => {
      if (!first) return new Promise<Response>(() => {});
      first = false;
      return Promise.resolve(fromServer({ checkedAt: server - 30 * MINUTE }, server));
    });
    renderAge();
    expect(await screen.findByText("Updated 30 min ago")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5 * MINUTE);
    });
    expect(screen.getByText("Updated 35 min ago")).toBeInTheDocument();
  });
});
