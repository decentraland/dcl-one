import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

import MapGridAge from "./MapGridAge";

const MINUTE = 60_000;

/** Answers the map API's tiles info with `answer`, and records the reads. */
function serveTilesInfo(answer: () => Promise<Response>) {
  const reads: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      reads.push(url);
      return answer();
    }),
  );
  return reads;
}

function renderAge() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MapGridAge />
    </QueryClientProvider>,
  );
}

/** The server's clock on a whole second, as a `Date` header carries it. */
const serverNow = () => Math.floor(Date.now() / 1000) * 1000;
const fromServer = (body: unknown, at: number) => Response.json(body, { headers: { date: new Date(at).toUTCString() } });

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
    ["a map API that cannot serve its grid", () => new Response(null, { status: 503 })],
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
