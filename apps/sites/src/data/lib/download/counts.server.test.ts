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

  it("is null on an error status, an unreadable body or a failed request, and asks again next time", async () => {
    answer({ values: [] }, 503);
    expect(await loadDownloadCounts()).toBeNull();
    answer({ values: "many" });
    expect(await loadDownloadCounts()).toBeNull();
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    expect(await loadDownloadCounts()).toBeNull();
    answer({ values: [["Windows", 0]] });
    expect(await loadDownloadCounts()).toBeNull();
    answer({ values: [["Windows", 1500]] });
    expect(await loadDownloadCounts()).toBe("+1K");
    expect(fetchMock).toHaveBeenCalledTimes(5);
  });

  it("formats as the reference does", () => {
    expect(formatToShorthand(999)).toBe("+999");
    expect(formatToShorthand(438744)).toBe("+438K");
    expect(formatToShorthand(2_500_000)).toBe("+2M");
  });
});
