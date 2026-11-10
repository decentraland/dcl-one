import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/data/lib/catalyst/landings/download-counts.server", () => ({
  loadDownloadsLabel: vi.fn(),
}));

import { loadDownloadsLabel } from "@/data/lib/catalyst/landings/download-counts.server";
import { loader } from "./landings.download";

describe("/download loader", () => {
  beforeEach(() => {
    vi.mocked(loadDownloadsLabel).mockReset();
    vi.mocked(loadDownloadsLabel).mockResolvedValue("+438K");
  });

  it.each([
    [
      "Windows NT 10.0; Win64; x64",
      { kind: "desktop", os: "windows", arch: "amd64" },
    ],
    ["Android 15; Pixel 9", { kind: "mobile", os: "android" }],
    ["iPhone; CPU iPhone OS 18_0 like Mac OS X", { kind: "mobile", os: "ios" }],
  ])(
    "should provide the detected platform for %s",
    async (userAgent, platform) => {
      const result = await loader({
        request: new Request("https://sites.example/download", {
          headers: { "user-agent": userAgent },
        }),
      } as never);

      expect(result).toEqual({ platform, downloads: "+438K" });
    },
  );
});
