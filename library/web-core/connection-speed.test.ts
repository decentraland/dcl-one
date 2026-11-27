// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";

import { measureDownlinkMbps, PROBE_BYTES, PROBE_URL } from "./connection-speed";

const CHUNK = 65_536;

// A body delivered in `chunks` reads, the clock advancing `msPerChunk` (or, per read, `msPerChunk(n)`) between
// reads; `failAfter` reads in, the stream errors the way an aborted fetch does. No read-ahead, so the clock moves
// only on reads. `length: false` streams it with no Content-Length, as a chunked proxy does.
const ON_DEMAND = { highWaterMark: 0 };
function serve(chunks: number, msPerChunk: number | ((n: number) => number), failAfter?: number, { length = true } = {}) {
  let now = 0;
  vi.spyOn(performance, "now").mockImplementation(() => now);
  let sent = 0;
  const body = new ReadableStream<Uint8Array>(
    {
      pull(controller) {
        if (failAfter !== undefined && sent === failAfter) {
          controller.error(new DOMException("timed out", "TimeoutError"));
          return;
        }
        if (sent === chunks) {
          controller.close();
          return;
        }
        sent += 1;
        now += typeof msPerChunk === "number" ? msPerChunk : msPerChunk(sent);
        controller.enqueue(new Uint8Array(CHUNK));
      },
    },
    ON_DEMAND,
  );
  const headers: Record<string, string> = length ? { "content-length": String(chunks * CHUNK) } : {};
  const fetchMock = vi.fn(() => Promise.resolve(new Response(body, { headers })));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("measureDownlinkMbps", () => {
  it("downloads the probe uncached and times the second half of the body", async () => {
    // 8 chunks, 50 ms apart: the first 4 cover the midpoint, the other 4 are the timed 200 ms window.
    const fetchMock = serve(8, 50);
    expect(await measureDownlinkMbps()).toBeCloseTo((4 * CHUNK * 8) / 1e6 / 0.2, 2);
    expect(fetchMock).toHaveBeenCalledWith(PROBE_URL, expect.objectContaining({ cache: "no-store" }));
  });

  it("ignores the slow start: the same link reads the same however slow the first half was", async () => {
    serve(8, 50);
    const even = await measureDownlinkMbps();
    let now = 0;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    let sent = 0;
    const body = new ReadableStream<Uint8Array>(
      {
        pull(controller) {
          if (sent === 8) return controller.close();
          sent += 1;
          now += sent <= 4 ? 400 : 50;
          controller.enqueue(new Uint8Array(CHUNK));
        },
      },
      ON_DEMAND,
    );
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response(body, { headers: { "content-length": String(8 * CHUNK) } }))));
    expect(await measureDownlinkMbps()).toBeCloseTo(even as number, 5);
  });

  it("times the second half of the probe's known size when a chunked proxy drops the Content-Length", async () => {
    expect(8 * CHUNK).toBe(PROBE_BYTES);
    serve(8, 50);
    const even = await measureDownlinkMbps();
    // A cold connection: a slow first half, then the same link speed. Without the size, the slow start would count.
    serve(8, (n) => (n <= 4 ? 400 : 50), undefined, { length: false });
    expect(await measureDownlinkMbps()).toBeCloseTo(even as number, 5);
  });

  it("reads a timed-out probe as a slow link, whether before or part-way through the body", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new DOMException("timed out", "TimeoutError"))));
    expect(await measureDownlinkMbps()).toBe(0);
    serve(8, 50, 5);
    expect(await measureDownlinkMbps()).toBe(0);
  });

  it("reads a failed, rejected or truncated probe as unknown", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new TypeError("Failed to fetch"))));
    expect(await measureDownlinkMbps()).toBeNull();
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response("", { status: 404 }))));
    expect(await measureDownlinkMbps()).toBeNull();
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response(new Uint8Array(10), { headers: { "content-length": "10" } }))));
    expect(await measureDownlinkMbps()).toBeNull();
  });
});
