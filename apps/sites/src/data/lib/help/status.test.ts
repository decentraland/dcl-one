import { focusManager, QueryClient, QueryObserver } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

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

function bodyResponse(status: number) {
  const cancel = vi.fn(() => Promise.resolve());
  const response = new Response(new ReadableStream({ cancel }), { status });
  return { response, cancel };
}

/** A fetch that never answers, and rejects the way fetch does when its signal aborts. */
const hangingFetch = vi.fn(
  (_url: string | URL | Request, init?: RequestInit) =>
    new Promise<Response>((_resolve, reject) => {
      init?.signal?.addEventListener("abort", () => reject(init.signal?.reason), { once: true });
    }),
);

describe("pingService", () => {
  it("reports up on an OK answer and cancels the unread body", async () => {
    const { response, cancel } = bodyResponse(200);
    const fetchImpl = vi.fn(async () => response);
    await expect(pingService(URL_A, undefined, fetchImpl)).resolves.toBe("ok");
    expect(fetchImpl).toHaveBeenCalledWith(URL_A, expect.objectContaining({ cache: "no-store" }));
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("reports down on a non-OK answer and cancels its body", async () => {
    const { response, cancel } = bodyResponse(503);
    await expect(pingService(URL_A, undefined, async () => response)).resolves.toBe("down");
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("reports down when the request fails", async () => {
    const fetchImpl = vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    });
    await expect(pingService(URL_A, undefined, fetchImpl)).resolves.toBe("down");
  });

  it("reports down when no answer comes within the timeout, and aborts the request", async () => {
    hangingFetch.mockClear();
    await expect(pingService(URL_A, undefined, hangingFetch, 10)).resolves.toBe("down");
    const signal = hangingFetch.mock.calls[0][1]?.signal;
    expect(signal?.aborted).toBe(true);
  });

  it("rejects instead of reporting down when the caller aborts", async () => {
    const controller = new AbortController();
    const pending = pingService(URL_A, controller.signal, hangingFetch, 60_000);
    controller.abort(new DOMException("unmounted", "AbortError"));
    await expect(pending).rejects.toThrow("unmounted");
  });
});

describe("overallState", () => {
  it("is ok when every service is up", () => {
    expect(overallState([{ name: "A", state: "ok" }, { name: "B", state: "ok" }])).toBe("ok");
  });

  it("is down when every service is down", () => {
    expect(overallState([{ name: "A", state: "down" }, { name: "B", state: "down" }])).toBe("down");
  });

  it("is slow when some services are down", () => {
    expect(overallState([{ name: "A", state: "ok" }, { name: "B", state: "down" }])).toBe("slow");
  });
});

describe("parseStatusServices", () => {
  it("keeps a configured list of https services", () => {
    const raw = JSON.stringify([{ name: " A ", url: URL_A }, { name: "B", url: URL_B }]);
    expect(parseStatusServices(raw)).toEqual([
      { name: "A", url: URL_A },
      { name: "B", url: URL_B },
    ]);
  });

  it.each([
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
  ])("falls back to the production services when the value is %s", (_label, raw) => {
    expect(parseStatusServices(raw)).toBe(DEFAULT_STATUS_SERVICES);
  });
});

describe("parseStatusPageUrl", () => {
  it("keeps an https URL", () => {
    expect(parseStatusPageUrl(" https://status.example/ ")).toBe("https://status.example/");
  });

  it.each([undefined, "", "javascript:alert(1)", "http://status.example/"])(
    "falls back to the production status page for %s",
    (raw) => {
      expect(parseStatusPageUrl(raw)).toBe(DEFAULT_STATUS_PAGE_URL);
    },
  );
});

describe("serviceStatusQuery", () => {
  it("keys the cache by every service's name and URL", () => {
    const a = serviceStatusKeys.all([{ name: "A", url: URL_A }]);
    expect(serviceStatusKeys.all([{ name: "A", url: URL_B }])).not.toEqual(a);
    expect(serviceStatusKeys.all([{ name: "B", url: URL_A }])).not.toEqual(a);
  });

  it("declares its staleTime and reports each service in order", async () => {
    const fetchImpl = vi.fn(async (url: string | URL | Request) => new Response(null, { status: url === URL_A ? 200 : 500 }));
    const options = serviceStatusQuery(
      [
        { name: "A", url: URL_A },
        { name: "B", url: URL_B },
      ],
      fetchImpl,
    );
    expect(options.staleTime).toBe(SERVICE_STATUS_STALE_MS);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    await expect(client.fetchQuery(options)).resolves.toEqual([
      { name: "A", state: "ok" },
      { name: "B", state: "down" },
    ]);
  });

  it("does not ping the services again when the window regains focus", async () => {
    const fetchImpl = vi.fn(async () => new Response(null, { status: 200 }));
    const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 0 } } });
    const options = serviceStatusQuery([{ name: "A", url: URL_A }], fetchImpl);
    // Stale at once, so only refetchOnWindowFocus decides whether a focus pings again.
    const observer = new QueryObserver(client, { ...options, staleTime: 0 });
    client.mount();
    const unsubscribe = observer.subscribe(() => {});
    try {
      await vi.waitFor(() => expect(observer.getCurrentResult().isSuccess).toBe(true));
      expect(fetchImpl).toHaveBeenCalledTimes(1);
      focusManager.setFocused(false);
      focusManager.setFocused(true);
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    } finally {
      unsubscribe();
      client.unmount();
      focusManager.setFocused(undefined);
    }
  });
});
