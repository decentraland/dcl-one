// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  deliverInstaller,
  estimateHoldMs,
  ESTIMATE_MAX_HOLD_MS,
  ESTIMATE_MIN_HOLD_MS,
  FALLBACK_HOLD_MS,
} from "./delivery";

const URL_WITH_QUERY =
  "https://gateway.example/anonymous/decentraland.exe?anon_user_id=3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f&position=10%2C20&referrer=partner";

let anchors: { href: string; download: string | null }[];

beforeEach(() => {
  anchors = [];
  vi.useFakeTimers();
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0),
  );
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    anchors.push({ href: this.href, download: this.getAttribute("download") });
  });
  URL.createObjectURL = vi.fn(() => "blob:local/1");
  URL.revokeObjectURL = vi.fn();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function streamResponse(
  bytes: number,
  headers: Record<string, string> = {},
): Response {
  return new Response(new Uint8Array(bytes), {
    status: 200,
    headers: { "content-length": String(bytes), ...headers },
  });
}

describe("deliverInstaller on Windows", () => {
  it("streams the file, reports progress and returns the gateway request id and bytes", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => streamResponse(10, { "x-request-id": "req-1" })),
    );
    const progress: number[] = [];

    const result = await deliverInstaller({
      url: URL_WITH_QUERY,
      filename: "decentraland.exe",
      os: "windows",
      signal: new AbortController().signal,
      onProgress: (p) => progress.push(p),
    });

    expect(result).toEqual({
      deliveryMode: "streamed",
      gatewayRequestId: "req-1",
      bytesTransferred: 10,
    });
    expect(progress.at(-1)).toBe(99);
    expect(anchors).toEqual([
      { href: "blob:local/1", download: "decentraland.exe" },
    ]);
  });

  it("retries the same URL, query included, through a native anchor when the stream fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      }),
    );

    const pending = deliverInstaller({
      url: URL_WITH_QUERY,
      filename: "decentraland.exe",
      os: "windows",
      signal: new AbortController().signal,
    });
    await vi.advanceTimersByTimeAsync(FALLBACK_HOLD_MS);

    await expect(pending).resolves.toEqual({ deliveryMode: "anchor_fallback" });
    expect(anchors).toHaveLength(1);
    expect(anchors[0]!.href).toBe(URL_WITH_QUERY);
  });

  it("treats a non-2xx response as a stream failure", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(null, { status: 502 })),
    );

    const pending = deliverInstaller({
      url: URL_WITH_QUERY,
      filename: "decentraland.exe",
      os: "windows",
      signal: new AbortController().signal,
    });
    await vi.advanceTimersByTimeAsync(FALLBACK_HOLD_MS);

    await expect(pending).resolves.toEqual({ deliveryMode: "anchor_fallback" });
    expect(anchors[0]!.href).toBe(URL_WITH_QUERY);
  });

  it("rethrows when the run was aborted instead of falling back", async () => {
    const controller = new AbortController();
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        controller.abort();
        throw new DOMException("aborted", "AbortError");
      }),
    );

    await expect(
      deliverInstaller({
        url: URL_WITH_QUERY,
        filename: "decentraland.exe",
        os: "windows",
        signal: controller.signal,
      }),
    ).rejects.toThrow("aborted");
    expect(anchors).toHaveLength(0);
  });
});

describe("deliverInstaller on macOS", () => {
  it("uses a native anchor on the gateway URL, without a download attribute cross-origin", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(null, {
            status: 200,
            headers: { "content-length": "1" },
          }),
      ),
    );
    const url = "https://gateway.example/id-1/decentraland.dmg?anon_user_id=x";

    const pending = deliverInstaller({
      url,
      filename: "decentraland.dmg",
      os: "macos",
      signal: new AbortController().signal,
    });
    await vi.advanceTimersByTimeAsync(ESTIMATE_MAX_HOLD_MS);

    await expect(pending).resolves.toEqual({ deliveryMode: "anchor_native" });
    expect(anchors).toEqual([{ href: url, download: null }]);
  });
});

describe("estimateHoldMs", () => {
  it("clamps the hold between the minimum and the maximum", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () => new Response(null, { headers: { "content-length": "1" } }),
      ),
    );
    await expect(estimateHoldMs("https://x.example/a")).resolves.toBe(
      ESTIMATE_MIN_HOLD_MS,
    );

    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(null, {
            headers: { "content-length": String(10 ** 10) },
          }),
      ),
    );
    await expect(estimateHoldMs("https://x.example/a")).resolves.toBe(
      ESTIMATE_MAX_HOLD_MS,
    );
  });

  it("falls back to the minimum when the HEAD fails or has no size", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("cors");
      }),
    );
    await expect(estimateHoldMs("https://x.example/a")).resolves.toBe(
      ESTIMATE_MIN_HOLD_MS,
    );

    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(null)),
    );
    await expect(estimateHoldMs("https://x.example/a")).resolves.toBe(
      ESTIMATE_MIN_HOLD_MS,
    );
  });
});
