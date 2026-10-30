import { act } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import FreshnessNotice from "./FreshnessNotice";
import { noteCopy, olderCopy, readCopy, resetCopies, useCopyAsOf, type CopyDomain } from "../data/copyAsOf";

const NOW = Date.parse("2026-10-02T12:00:00Z");
const HOUR = 3_600_000;

const stamp = (ageMs: number, staleAfterSecs?: number) => ({
  as_of: new Date(NOW - ageMs).toISOString(),
  ...(staleAfterSecs === undefined ? {} : { stale_after_secs: staleAfterSecs }),
  data: [],
});

describe("FreshnessNotice", () => {
  afterEach(() => resetCopies());

  it("says how old a mirrored copy is once it is past the budget its node sent, and nothing before", () => {
    const html = (body: unknown) => renderToStaticMarkup(<FreshnessNotice copy={readCopy(body)} now={NOW} />);
    expect(html(stamp(3 * HOUR, 7200))).toContain("as of 3 hours ago");
    expect(html(stamp(26 * HOUR, 7200))).toContain("as of 1 day ago");
    expect(html(stamp(HOUR, 7200))).toBe("");
    expect(html(stamp(3 * HOUR, 4 * 3600)), "a node with a longer interval sends a longer budget").toBe("");
    expect(html(stamp(10 * 60_000, 300))).toContain("as of 10 minutes ago");
    expect(html(stamp(HOUR)), "no budget: said at once").toContain("as of 1 hour ago");
    expect(html({ data: [] })).toBe("");
    const failed = renderToStaticMarkup(<FreshnessNotice failed onRetry={() => {}} now={NOW} />);
    expect(failed).toContain("Showing saved results");
    expect(failed).not.toContain("copy-as-of");
  });

  it("a read over two copies is as old as the older and stale as soon as either is", () => {
    const fresh = { asOf: NOW - HOUR, staleAfterMs: 2 * HOUR };
    const older = { asOf: NOW - 3 * HOUR, staleAfterMs: 48 * HOUR };
    expect(olderCopy(fresh, older)).toEqual({ asOf: NOW - 3 * HOUR, staleAfterMs: 4 * HOUR });
    expect(olderCopy(null, fresh)).toEqual(fresh);
    expect(olderCopy(undefined, null)).toBeNull();
  });

  it("follows the stamps the fetchers note, the oldest of a panel's domains", async () => {
    const domains: readonly CopyDomain[] = ["places", "worlds"];
    function Panel() {
      return <FreshnessNotice copy={useCopyAsOf(domains)} now={NOW} />;
    }
    const host = document.createElement("div");
    const root = createRoot(host);
    await act(async () => root.render(<Panel />));
    expect(host.textContent).toBe("");
    await act(async () => noteCopy("places", stamp(30 * 60_000, 7200)));
    expect(host.textContent).toBe("");
    await act(async () => noteCopy("worlds", stamp(5 * HOUR, 7200)));
    expect(host.querySelector('[data-testid="copy-as-of"]')?.textContent).toContain("as of 5 hours ago");
    await act(async () => noteCopy("worlds", stamp(5 * HOUR, 6 * 3600)));
    expect(host.textContent, "the budget alone changed").toBe("");
    await act(async () => noteCopy("worlds", { data: [] }));
    expect(host.textContent).toBe("");
    await act(async () => root.unmount());
  });
});
