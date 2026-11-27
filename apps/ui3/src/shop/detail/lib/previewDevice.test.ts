// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const probe = vi.hoisted(() => ({ measure: vi.fn<() => Promise<number | null>>() }));
vi.mock("@web-core/connection-speed", () => ({ measureDownlinkMbps: probe.measure }));

import { decidePreviewDevice, resetMeasuredLinkForTests } from "./previewDevice";
import type { RendererDecision } from "./pickRenderer";

function wide(width: "desktop" | "phone") {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: width === "phone" && query.includes("max-width"), media: query }));
}

beforeEach(() => {
  resetMeasuredLinkForTests();
  probe.measure.mockReset();
  vi.stubGlobal("WebGLRenderingContext", function WebGLRenderingContext() {});
  wide("desktop");
});

afterEach(() => vi.unstubAllGlobals());

const decide = () => {
  const seen: RendererDecision[] = [];
  const stop = decidePreviewDevice((d) => seen.push(d));
  return { seen, stop };
};

describe("decidePreviewDevice", () => {
  describe("when the device alone settles it", () => {
    it("answers at once without the probe download", () => {
      wide("phone");
      const { seen } = decide();
      expect(seen).toEqual([{ renderer: "lite", reason: "mobile" }]);
      expect(probe.measure).not.toHaveBeenCalled();
    });
  });

  describe("when only the link is left to decide", () => {
    it("draws full on a fast link, measured once for every preview on the page", async () => {
      probe.measure.mockResolvedValue(80);
      const first = decide();
      const second = decide();
      expect(first.seen).toEqual([]);
      await vi.waitFor(() => expect(second.seen).toHaveLength(1));
      expect(first.seen).toEqual([{ renderer: "full", reason: "connection-ok" }]);
      expect(probe.measure).toHaveBeenCalledOnce();
    });

    it("draws lite on a slow link", async () => {
      probe.measure.mockResolvedValue(4);
      const { seen } = decide();
      await vi.waitFor(() => expect(seen).toEqual([{ renderer: "lite", reason: "slow-connection" }]));
    });

    it("stays optimistic when the probe could not run", async () => {
      probe.measure.mockResolvedValue(null);
      const { seen } = decide();
      await vi.waitFor(() => expect(seen).toEqual([{ renderer: "full", reason: "optimistic-default" }]));
    });

    it("drops the answer for a caller that is gone", async () => {
      let answer: (mbps: number) => void = () => {};
      probe.measure.mockReturnValue(new Promise((resolve) => (answer = resolve)));
      const { seen, stop } = decide();
      stop();
      answer(80);
      await Promise.resolve();
      await Promise.resolve();
      expect(seen).toEqual([]);
    });
  });
});
