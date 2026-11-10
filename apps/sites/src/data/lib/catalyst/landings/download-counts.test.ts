import { afterEach, describe, expect, it, vi } from "vitest";

import {
  downloadCountsMemo,
  DOWNLOADS_FALLBACK_LABEL,
  fetchDownloadTotal,
  formatDownloads,
  loadDownloadsLabel,
} from "./download-counts.server";

const URL = "https://counts.example/downloads.json";
const ok = (body: unknown) =>
  vi.fn(async () => new Response(JSON.stringify(body), { status: 200 }));

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  downloadCountsMemo.reset();
});

describe("fetchDownloadTotal", () => {
  it("sums the platform downloads", async () => {
    const fetchImpl = ok({
      values: [
        ["Windows", 300_000],
        ["Mac", 150_500],
      ],
    });
    await expect(fetchDownloadTotal(URL, fetchImpl)).resolves.toBe(450_500);
    expect(fetchImpl).toHaveBeenCalledWith(
      URL,
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("returns null without a URL, without fetching", async () => {
    const fetchImpl = vi.fn();
    await expect(fetchDownloadTotal(undefined, fetchImpl)).resolves.toBeNull();
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it.each([
    ["an HTTP error", vi.fn(async () => new Response("", { status: 503 }))],
    [
      "a network error",
      vi.fn(async () => {
        throw new TypeError("fetch failed");
      }),
    ],
    ["a malformed body", ok({ rows: [] })],
    ["a non-numeric count", ok({ values: [["Windows", "many"]] })],
    ["a zero total", ok({ values: [] })],
  ])("returns null on %s", async (_, fetchImpl) => {
    await expect(
      fetchDownloadTotal(URL, fetchImpl as typeof fetch),
    ).resolves.toBeNull();
  });

  it("gives up after the timeout", async () => {
    const hang = vi.fn(
      (_: unknown, init?: RequestInit) =>
        new Promise<Response>((_, reject) =>
          init?.signal?.addEventListener("abort", () =>
            reject(init.signal?.reason),
          ),
        ),
    );
    await expect(
      fetchDownloadTotal(URL, hang as unknown as typeof fetch, 5),
    ).resolves.toBeNull();
  });
});

describe("formatDownloads", () => {
  it("shortens as source does", () => {
    expect(formatDownloads(2_345_678)).toBe("+2M");
    expect(formatDownloads(450_500)).toBe("+450K");
    expect(formatDownloads(999)).toBe("+999");
  });
});

describe("loadDownloadsLabel", () => {
  it("shows the source's static label when DOWNLOAD_COUNTS_URL is unset", async () => {
    vi.stubEnv("DOWNLOAD_COUNTS_URL", "");
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);

    await expect(loadDownloadsLabel()).resolves.toBe(DOWNLOADS_FALLBACK_LABEL);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("formats the configured endpoint's total and serves it from the memo", async () => {
    vi.stubEnv("DOWNLOAD_COUNTS_URL", URL);
    const fetchSpy = ok({ values: [["Windows", 1_200_000]] });
    vi.stubGlobal("fetch", fetchSpy);

    await expect(loadDownloadsLabel()).resolves.toBe("+1M");
    await expect(loadDownloadsLabel()).resolves.toBe("+1M");
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("falls back on failure and retries on the next request", async () => {
    vi.stubEnv("DOWNLOAD_COUNTS_URL", URL);
    const fetchSpy = vi
      .fn()
      .mockResolvedValueOnce(new Response("", { status: 500 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ values: [["Mac", 5_000]] })),
      );
    vi.stubGlobal("fetch", fetchSpy);

    await expect(loadDownloadsLabel()).resolves.toBe(DOWNLOADS_FALLBACK_LABEL);
    await expect(loadDownloadsLabel()).resolves.toBe("+5K");
  });
});
