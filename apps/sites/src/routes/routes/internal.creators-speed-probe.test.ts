import { describe, expect, it } from "vitest";

import { loader } from "./internal.creators-speed-probe";

describe("the creators speed probe", () => {
  it("should answer a fixed, uncached download large enough to time", async () => {
    const response = loader();
    const body = new Uint8Array(await response.arrayBuffer());
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(body.byteLength).toBe(512 * 1024);
    expect(Number(response.headers.get("content-length"))).toBe(body.byteLength);
  });
});
