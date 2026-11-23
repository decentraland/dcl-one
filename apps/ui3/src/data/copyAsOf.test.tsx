import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { staleCopyAge } from "@web-core/freshness/copyAsOf";
import { noteCopy, resetCopies, useCopyAsOf, type CopyDomain } from "./copyAsOf";

const NOW = Date.parse("2026-10-02T12:00:00Z");
const HOUR = 3_600_000;

const stamp = (ageMs: number, staleAfterSecs?: number) => ({
  as_of: new Date(NOW - ageMs).toISOString(),
  ...(staleAfterSecs === undefined ? {} : { stale_after_secs: staleAfterSecs }),
  data: [],
});

describe("copyAsOf", () => {
  afterEach(() => resetCopies());

  it("follows the stamps the fetchers note, the oldest of a panel's domains", async () => {
    const domains: readonly CopyDomain[] = ["places", "worlds"];
    function Panel() {
      return <output>{staleCopyAge(useCopyAsOf(domains), NOW)}</output>;
    }
    const host = document.createElement("div");
    const root = createRoot(host);
    await act(async () => root.render(<Panel />));
    expect(host.textContent).toBe("");
    await act(async () => noteCopy("places", stamp(30 * 60_000, 7200)));
    expect(host.textContent).toBe("");
    await act(async () => noteCopy("worlds", stamp(5 * HOUR, 7200)));
    expect(host.textContent).toBe("5 hours");
    await act(async () => noteCopy("worlds", stamp(5 * HOUR, 6 * 3600)));
    expect(host.textContent, "the budget alone changed").toBe("");
    await act(async () => noteCopy("worlds", { data: [] }));
    expect(host.textContent).toBe("");
    await act(async () => root.unmount());
  });
});
