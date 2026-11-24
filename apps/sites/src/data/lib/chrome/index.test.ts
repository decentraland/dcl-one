import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { chromeKeys, chromeNotificationsQuery, type ChromeNotificationsFetcher } from "./index";

describe("chrome notification queries", () => {
  let client: QueryClient;

  beforeEach(() => {
    client = new QueryClient();
  });

  afterEach(() => {
    client.clear();
    vi.resetAllMocks();
  });

  describe("when the query is cancelled through the normalized account key", () => {
    let signal: AbortSignal | undefined;
    let pending: Promise<unknown>;

    beforeEach(async () => {
      signal = undefined;
      const fetcher = vi.fn((input?: AbortSignal) => {
        signal = input;
        return new Promise((_, reject) =>
          input?.addEventListener("abort", () => reject(input.reason)),
        );
      });
      pending = client.fetchQuery(chromeNotificationsQuery("0xA", fetcher));
      await client.cancelQueries({ queryKey: chromeKeys.notifications("0xa") });
    });

    it("should reject the pending fetch", async () => {
      await expect(pending).rejects.toBeDefined();
    });

    it("should forward the cancellation signal to the fetcher", async () => {
      await pending.catch(() => undefined);
      expect(signal?.aborted).toBe(true);
    });

    it("should not cache data under another account", async () => {
      await pending.catch(() => undefined);
      expect(
        client.getQueryData(chromeKeys.notifications("0xb")),
      ).toBeUndefined();
    });
  });

  describe("when the identity or the fetcher is missing", () => {
    let fetcher: Mock<ChromeNotificationsFetcher>;

    beforeEach(() => {
      fetcher = vi.fn();
    });

    describe("and the identity is missing", () => {
      it("should refuse with an unavailable error", async () => {
        await expect(
          client.fetchQuery(chromeNotificationsQuery(null, fetcher)),
        ).rejects.toThrow("unavailable");
      });

      it("should not issue a request", async () => {
        await client.fetchQuery(chromeNotificationsQuery(null, fetcher)).catch(() => undefined);
        expect(fetcher).not.toHaveBeenCalled();
      });
    });

    describe("and the fetcher is unavailable", () => {
      it("should refuse with an unavailable error", async () => {
        await expect(
          client.fetchQuery(chromeNotificationsQuery("0xa")),
        ).rejects.toThrow("unavailable");
      });
    });
  });
});
