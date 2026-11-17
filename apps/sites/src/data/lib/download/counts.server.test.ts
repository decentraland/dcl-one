import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { formatToShorthand, loadDownloadCounts, resetDownloadCounts } from "./counts.server";

const COUNTS_URL = "https://cdn-data.test/public/current/explorer-alpha-downloads.json";
const fetchMock = vi.fn();

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  vi.stubEnv("DOWNLOAD_COUNTS_URL", COUNTS_URL);
  resetDownloadCounts();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
  vi.useRealTimers();
  fetchMock.mockReset();
});

const answer = (body: unknown, status = 200) => fetchMock.mockResolvedValueOnce(new Response(JSON.stringify(body), { status }));

describe("the Explorer's download total", () => {
  it("sums every platform's downloads into the shorthand and reads once a minute", async () => {
    answer({ values: [["Windows", 397303], ["Mac", 41362], ["None", 58], ["Mac Intel", 21]] });
    expect(await loadDownloadCounts()).toBe("+438K");
    expect(await loadDownloadCounts()).toBe("+438K");
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock.mock.calls[0]![0]).toBe(COUNTS_URL);
  });

  it("is null without a configured URL, and asks nothing", async () => {
    vi.stubEnv("DOWNLOAD_COUNTS_URL", "");
    expect(await loadDownloadCounts()).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("is null on an error status, an unreadable body, a failed request or an empty total", async () => {
    for (const fail of [
      () => answer({ values: [] }, 503),
      () => answer({ values: "many" }),
      () => fetchMock.mockRejectedValueOnce(new Error("offline")),
      () => answer({ values: [["Windows", 0]] }),
    ]) {
      resetDownloadCounts();
      fail();
      expect(await loadDownloadCounts()).toBeNull();
    }
    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it("gives the request a 2 s deadline", async () => {
    vi.useFakeTimers();
    fetchMock.mockImplementationOnce(
      (_url: string, init: RequestInit) =>
        new Promise((_resolve, reject) => {
          init.signal!.addEventListener("abort", () => reject(init.signal!.reason), { once: true });
        }),
    );
    const pending = loadDownloadCounts();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(await pending).toBeNull();
    expect((fetchMock.mock.calls[0]![1] as RequestInit).signal!.aborted).toBe(true);
  });

  it("holds a failed read for 30 s, then asks again", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    answer({ values: [] }, 503);
    expect(await loadDownloadCounts()).toBeNull();
    vi.advanceTimersByTime(29_999);
    expect(await loadDownloadCounts()).toBeNull();
    expect(fetchMock).toHaveBeenCalledOnce();
    // Past both the hold and the stale period, so the next call waits for its own read.
    vi.advanceTimersByTime(90_001);
    answer({ values: [["Windows", 1500]] });
    expect(await loadDownloadCounts()).toBe("+1K");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("keeps the last total read when a later read fails", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    answer({ values: [["Windows", 1500]] });
    expect(await loadDownloadCounts()).toBe("+1K");
    vi.advanceTimersByTime(120_001);
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    expect(await loadDownloadCounts()).toBe("+1K");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("formats as the reference does", () => {
    expect(formatToShorthand(999)).toBe("+999");
    expect(formatToShorthand(438744)).toBe("+438K");
    expect(formatToShorthand(2_500_000)).toBe("+2M");
  });
});
