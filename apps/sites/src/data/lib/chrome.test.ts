import { describe, expect, it, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { chromeKeys, chromeNotificationsQuery } from "./chrome";

describe("chrome notification queries", () => {
  it("normalizes identity and forwards the query cancellation signal", async () => {
    let signal: AbortSignal | undefined;
    const fetcher = vi.fn((input?: AbortSignal) => {
      signal = input;
      return new Promise((_, reject) =>
        input?.addEventListener("abort", () => reject(input.reason)),
      );
    });
    const client = new QueryClient();
    const pending = client.fetchQuery(chromeNotificationsQuery("0xA", fetcher));
    await client.cancelQueries({ queryKey: chromeKeys.notifications("0xa") });
    await expect(pending).rejects.toBeDefined();
    expect(signal?.aborted).toBe(true);
    expect(
      client.getQueryData(chromeKeys.notifications("0xb")),
    ).toBeUndefined();
    client.clear();
  });
  it("refuses missing identity and unavailable fetcher without issuing a request", async () => {
    const fetcher = vi.fn();
    const client = new QueryClient();
    await expect(
      client.fetchQuery(chromeNotificationsQuery(null, fetcher)),
    ).rejects.toThrow("unavailable");
    await expect(
      client.fetchQuery(chromeNotificationsQuery("0xa")),
    ).rejects.toThrow("unavailable");
    expect(fetcher).not.toHaveBeenCalled();
    client.clear();
  });
});
