import { describe, expect, it } from "vitest";

import { applyFrameIsolation } from "./frame-isolation";

const headersFor = (fetchHeaders: Record<string, string>) => {
  const headers = new Headers();
  applyFrameIsolation(new Request("https://site.test/create", { headers: fetchHeaders }), headers);
  return headers;
};

describe("applyFrameIsolation", () => {
  it("gives a same-origin framed document COEP so the Explorer's isolated /play can show it", () => {
    const headers = headersFor({ "sec-fetch-dest": "iframe", "sec-fetch-site": "same-origin" });
    expect(headers.get("cross-origin-embedder-policy")).toBe("credentialless");
    expect(headers.get("vary")).toBe("Sec-Fetch-Dest, Sec-Fetch-Site");
  });

  it("leaves top-level pages and cross-site frames alone, varying the cache either way", () => {
    const requests: Record<string, string>[] = [
      { "sec-fetch-dest": "document", "sec-fetch-site": "same-origin" },
      { "sec-fetch-dest": "iframe", "sec-fetch-site": "cross-site" },
      {},
    ];
    for (const fetchHeaders of requests) {
      const headers = headersFor(fetchHeaders);
      expect(headers.get("cross-origin-embedder-policy")).toBeNull();
      expect(headers.get("vary")).toBe("Sec-Fetch-Dest, Sec-Fetch-Site");
    }
  });
});
