import { PROBE_BYTES } from "@web-core/connection-speed";
import { describe, expect, it } from "vitest";

import { loader } from "./internal.creators-speed-probe";

describe("the creators speed probe", () => {
  it("should answer a fixed, uncached download large enough to time, which no proxy may re-encode", async () => {
    const response = loader();
    const body = new Uint8Array(await response.arrayBuffer());
    expect(response.headers.get("cache-control")).toBe("no-store, no-transform");
    expect(PROBE_BYTES).toBe(512 * 1024);
    expect(body.byteLength).toBe(PROBE_BYTES);
    expect(Number(response.headers.get("content-length"))).toBe(body.byteLength);
  });
});
