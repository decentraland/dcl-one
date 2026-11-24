import { act, render, screen, waitFor } from "@testing-library/react";
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

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("MapGridAge", () => {
  it("says how long ago the map API last found its grid current, with a decorative clock", async () => {
    const reads = serveTilesInfo(async () => Response.json({ lastUpdatedAt: Date.now() - 600 * MINUTE, checkedAt: Date.now() - 12 * MINUTE }));
    render(<MapGridAge />);
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
    const { container } = render(<MapGridAge />);
    await waitFor(() => expect(reads).toHaveLength(1));
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });
});
