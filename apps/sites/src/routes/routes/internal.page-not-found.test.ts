import { describe, expect, it, vi } from "vitest";

import { action } from "./internal.page-not-found";

function post(body: string): Request {
  return new Request("https://example.org/internal/page-not-found", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });
}

function call(request: Request) {
  return action({ request } as unknown as Parameters<typeof action>[0]);
}

describe("internal.page-not-found action", () => {
  it("rejects non-POST, swallows garbage bodies silently and accepts a well-formed spa 404 report", async () => {
    expect((await call(new Request("https://example.org/internal/page-not-found"))).status).toBe(405);
    expect((await call(post("not json"))).status).toBe(204);
    expect((await call(post(JSON.stringify({ referrer: "x" })))).status).toBe(204);
    expect((await call(post(JSON.stringify({ path: "no-leading-slash" })))).status).toBe(204);
    const accepted = await call(
      post(JSON.stringify({ path: "/missing-page?q=1", referrer: "https://example.org/" })),
    );
    expect(accepted.status).toBe(202);
  });

  it("drops credential parameters from the path and the referrer before Segment", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(new Response(null, { status: 200 })));
    vi.stubGlobal("fetch", fetchMock);
    process.env.SEGMENT_WRITE_KEY = "key-1";
    try {
      const res = await call(
        post(JSON.stringify({ path: "/landings/cast-stream?token=k&q=1", referrer: "https://ref.example/?code=c" })),
      );
      expect(res.status).toBe(202);
      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
      const sent = JSON.parse(init.body as string).properties;
      expect(sent.path).toBe("/landings/cast-stream?q=1");
      expect(sent.referrer).toBe("https://ref.example/");
      expect(sent.spa).toBe(true);
    } finally {
      vi.unstubAllGlobals();
      delete process.env.SEGMENT_WRITE_KEY;
    }
  });
});
