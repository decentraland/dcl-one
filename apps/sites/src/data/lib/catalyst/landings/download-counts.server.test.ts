import { describe, expect, it, vi } from "vitest";
import {
  createDownloadsLabelLoader,
  fetchDownloadTotal,
  formatDownloads,
} from "./download-counts.server";

describe("public Explorer download aggregate", () => {
  it("sums the published platform counts without hardcoding the displayed total", async () => {
    const request = vi.fn().mockResolvedValue(
      Response.json({
        values: [
          ["Windows", 397323],
          ["Mac", 41368],
          ["None", 58],
          ["MacIntel", 21],
        ],
      }),
    );
    expect(await fetchDownloadTotal(request)).toBe(438770);
    expect(formatDownloads(438770)).toBe("+438K");
    expect(request).toHaveBeenCalledWith(
      "https://cdn-data.decentraland.org/public/current/explorer-alpha-downloads.json",
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });
  it.each([
    null,
    {},
    { values: [] },
    { values: [["x", -1]] },
    { values: [["x", "5"]] },
    { values: [["x", 0.5]] },
    {
      values: [
        ["x", Number.MAX_SAFE_INTEGER],
        ["y", 1],
      ],
    },
  ])("refuses malformed or empty aggregates", async (body) => {
    expect(
      await fetchDownloadTotal(vi.fn().mockResolvedValue(Response.json(body))),
    ).toBeNull();
  });
  it("cancels unread HTTP error bodies and retains the fallback when cleanup fails", async () => {
    const cancel = vi.fn().mockRejectedValue(new Error("closed"));
    const response = new Response(null, { status: 503 });
    Object.defineProperty(response, "body", { value: { cancel } });
    expect(
      await fetchDownloadTotal(vi.fn().mockResolvedValue(response)),
    ).toBeNull();
    expect(cancel).toHaveBeenCalledOnce();
  });
  it("handles transport and invalid JSON errors", async () => {
    expect(
      await fetchDownloadTotal(vi.fn().mockRejectedValue(new Error("offline"))),
    ).toBeNull();
    expect(
      await fetchDownloadTotal(
        vi.fn().mockResolvedValue(new Response("not JSON")),
      ),
    ).toBeNull();
  });
  it.each([
    [999, "+999"],
    [1000, "+1K"],
    [1000000, "+1M"],
  ])("formats the boundary %s", (total, label) =>
    expect(formatDownloads(total)).toBe(label),
  );
});

describe("download aggregate refresh policy", () => {
  it("shares a cold refresh and caches failure briefly before retrying", async () => {
    let now = 1000;
    const read = vi.fn().mockResolvedValueOnce(null).mockResolvedValue(438770);
    const load = createDownloadsLabelLoader(read, () => now);
    expect(await Promise.all([load(), load()])).toEqual(["+400K", "+400K"]);
    expect(read).toHaveBeenCalledOnce();
    now += 29999;
    expect(await load()).toBe("+400K");
    expect(read).toHaveBeenCalledOnce();
    now += 1;
    expect(await load()).toBe("+438K");
    expect(read).toHaveBeenCalledTimes(2);
  });
  it("retains the last good total when an expired refresh rejects", async () => {
    let now = 1000;
    const read = vi
      .fn()
      .mockResolvedValueOnce(438770)
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValue(439000);
    const load = createDownloadsLabelLoader(read, () => now);
    expect(await load()).toBe("+438K");
    now += 600000;
    expect(await load()).toBe("+438K");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    now += 29999;
    expect(await load()).toBe("+438K");
    expect(read).toHaveBeenCalledTimes(2);
    now += 1;
    expect(await load()).toBe("+438K");
    await vi.waitFor(async () => expect(await load()).toBe("+439K"));
  });

  it("serves a stale good total immediately while its refresh is pending", async () => {
    let now = 1000;
    let resolveRefresh: ((value: number | null) => void) | undefined;
    const read = vi
      .fn()
      .mockResolvedValueOnce(438770)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveRefresh = resolve;
          }),
      );
    const load = createDownloadsLabelLoader(read, () => now);

    expect(await load()).toBe("+438K");
    now += 600000;
    expect(await load()).toBe("+438K");
    expect(read).toHaveBeenCalledTimes(2);

    resolveRefresh?.(439000);
    await vi.waitFor(async () => expect(await load()).toBe("+439K"));
  });
});
