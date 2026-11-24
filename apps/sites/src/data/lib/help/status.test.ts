import { focusManager, QueryClient, QueryObserver } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

import {
  DEFAULT_STATUS_PAGE_URL,
  DEFAULT_STATUS_SERVICES,
  overallState,
  parseStatusPageUrl,
  parseStatusServices,
  pingService,
  serviceStatusKeys,
  serviceStatusQuery,
  SERVICE_STATUS_STALE_MS,
} from "./status";

const URL_A = "https://service-a.example/about";
const URL_B = "https://service-b.example/ping";

type FetchImpl = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

function bodyResponse(status: number) {
  const cancel = vi.fn(() => Promise.resolve());
  const response = new Response(new ReadableStream({ cancel }), { status });
  return { response, cancel };
}

/** A fetch that never answers, and rejects the way fetch does when its signal aborts. */
const makeHangingFetch = () =>
  vi.fn(
    (_url: string | URL | Request, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => reject(init.signal?.reason), { once: true });
      }),
  );

// Stale at once, so only refetchOnWindowFocus decides whether a focus pings again.
const staleObserver = (client: QueryClient, options: ReturnType<typeof serviceStatusQuery>) =>
  new QueryObserver(client, { ...options, staleTime: 0 });

afterEach(() => vi.resetAllMocks());

describe("pingService", () => {
  describe("when the service answers OK", () => {
    let fetchImpl: Mock<FetchImpl>;
    let cancel: ReturnType<typeof vi.fn>;
    let result: Promise<unknown>;

    beforeEach(() => {
      const body = bodyResponse(200);
      cancel = body.cancel;
      fetchImpl = vi.fn(async () => body.response);
      result = pingService(URL_A, undefined, fetchImpl);
    });

    it("should report up", async () => {
      await expect(result).resolves.toBe("ok");
    });

    it("should ask without the cache", async () => {
      await result;
      expect(fetchImpl).toHaveBeenCalledWith(URL_A, expect.objectContaining({ cache: "no-store" }));
    });

    it("should cancel the unread body", async () => {
      await result;
      expect(cancel).toHaveBeenCalledOnce();
    });
  });

  describe("when the service answers non-OK", () => {
    let cancel: ReturnType<typeof vi.fn>;
    let result: Promise<unknown>;

    beforeEach(() => {
      const body = bodyResponse(503);
      cancel = body.cancel;
      result = pingService(URL_A, undefined, async () => body.response);
    });

    it("should report down", async () => {
      await expect(result).resolves.toBe("down");
    });

    it("should cancel its body", async () => {
      await result;
      expect(cancel).toHaveBeenCalledOnce();
    });
  });

  describe("when the request fails", () => {
    let fetchImpl: Mock<FetchImpl>;

    beforeEach(() => {
      fetchImpl = vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      });
    });

    it("should report down", async () => {
      await expect(pingService(URL_A, undefined, fetchImpl)).resolves.toBe("down");
    });
  });

  describe("when no answer comes within the timeout", () => {
    let hangingFetch: ReturnType<typeof makeHangingFetch>;
    let result: Promise<unknown>;

    beforeEach(() => {
      hangingFetch = makeHangingFetch();
      result = pingService(URL_A, undefined, hangingFetch, 10);
    });

    it("should report down", async () => {
      await expect(result).resolves.toBe("down");
    });

    it("should abort the request", async () => {
      await result;
      const signal = hangingFetch.mock.calls[0][1]?.signal;
      expect(signal?.aborted).toBe(true);
    });
  });

  describe("when the caller aborts", () => {
    let controller: AbortController;
    let hangingFetch: ReturnType<typeof makeHangingFetch>;

    beforeEach(() => {
      controller = new AbortController();
      hangingFetch = makeHangingFetch();
    });

    it("should reject instead of reporting down", async () => {
      const pending = pingService(URL_A, controller.signal, hangingFetch, 60_000);
      controller.abort(new DOMException("unmounted", "AbortError"));
      await expect(pending).rejects.toThrow("unmounted");
    });
  });
});

describe("overallState", () => {
  describe("when every service is up", () => {
    it("should be ok", () => {
      expect(overallState([{ name: "A", state: "ok" }, { name: "B", state: "ok" }])).toBe("ok");
    });
  });

  describe("when every service is down", () => {
    it("should be down", () => {
      expect(overallState([{ name: "A", state: "down" }, { name: "B", state: "down" }])).toBe("down");
    });
  });

  describe("when some services are down", () => {
    it("should be slow", () => {
      expect(overallState([{ name: "A", state: "ok" }, { name: "B", state: "down" }])).toBe("slow");
    });
  });
});

describe("parseStatusServices", () => {
  describe("when the value is a list of https services", () => {
    it("should keep it", () => {
      expect(parseStatusServices(JSON.stringify([{ name: " A ", url: URL_A }, { name: "B", url: URL_B }]))).toEqual([
        { name: "A", url: URL_A },
        { name: "B", url: URL_B },
      ]);
    });
  });

  describe.each([
    ["unset", undefined],
    ["blank", "  "],
    ["not JSON", "{oops"],
    ["not a list", JSON.stringify({ name: "A", url: URL_A })],
    ["an empty list", "[]"],
    ["an entry without a name", JSON.stringify([{ url: URL_A }])],
    ["an http URL", JSON.stringify([{ name: "A", url: "http://service-a.example/about" }])],
    ["a non-URL", JSON.stringify([{ name: "A", url: "service-a" }])],
    ["a null entry", JSON.stringify([null])],
    ["two entries with the same name", JSON.stringify([{ name: "A", url: URL_A }, { name: " A", url: URL_B }])],
  ])("when the value is %s", (_label, raw) => {
    it("should fall back to the production services", () => {
      expect(parseStatusServices(raw)).toBe(DEFAULT_STATUS_SERVICES);
    });
  });
});

describe("parseStatusPageUrl", () => {
  describe("when the value is an https URL", () => {
    it("should keep it, trimmed", () => {
      expect(parseStatusPageUrl(" https://status.example/ ")).toBe("https://status.example/");
    });
  });

  describe.each([undefined, "", "javascript:alert(1)", "http://status.example/"])("when the value is %s", (raw) => {
    it("should fall back to the production status page", () => {
      expect(parseStatusPageUrl(raw)).toBe(DEFAULT_STATUS_PAGE_URL);
    });
  });
});

describe("serviceStatusQuery", () => {
  describe("when keying the cache", () => {
    let a: ReturnType<typeof serviceStatusKeys.all>;

    beforeEach(() => {
      a = serviceStatusKeys.all([{ name: "A", url: URL_A }]);
    });

    it("should key by every service's URL", () => {
      expect(serviceStatusKeys.all([{ name: "A", url: URL_B }])).not.toEqual(a);
    });

    it("should key by every service's name", () => {
      expect(serviceStatusKeys.all([{ name: "B", url: URL_A }])).not.toEqual(a);
    });
  });

  describe("when one service is up and another is down", () => {
    let options: ReturnType<typeof serviceStatusQuery>;

    beforeEach(() => {
      const fetchImpl = vi.fn(async (url: string | URL | Request) => new Response(null, { status: url === URL_A ? 200 : 500 }));
      options = serviceStatusQuery(
        [
          { name: "A", url: URL_A },
          { name: "B", url: URL_B },
        ],
        fetchImpl,
      );
    });

    it("should declare its staleTime", () => {
      expect(options.staleTime).toBe(SERVICE_STATUS_STALE_MS);
    });

    it("should report each service in order", async () => {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      await expect(client.fetchQuery(options)).resolves.toEqual([
        { name: "A", state: "ok" },
        { name: "B", state: "down" },
      ]);
    });
  });

  describe("when the window regains focus", () => {
    let fetchImpl: Mock<FetchImpl>;
    let client: QueryClient;
    let observer: ReturnType<typeof staleObserver>;
    let unsubscribe: () => void;

    beforeEach(() => {
      fetchImpl = vi.fn(async () => new Response(null, { status: 200 }));
      client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 0 } } });
      observer = staleObserver(client, serviceStatusQuery([{ name: "A", url: URL_A }], fetchImpl));
      client.mount();
      unsubscribe = observer.subscribe(() => {});
    });

    afterEach(() => {
      unsubscribe();
      client.unmount();
      focusManager.setFocused(undefined);
    });

    it("should not ping the services again", async () => {
      await vi.waitFor(() => expect(observer.getCurrentResult().isSuccess).toBe(true));
      expect(fetchImpl).toHaveBeenCalledTimes(1);
      focusManager.setFocused(false);
      focusManager.setFocused(true);
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
  });
});
