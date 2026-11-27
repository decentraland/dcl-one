import { describe, expect, it } from "vitest";
import { pickRenderer, type PreviewDevice } from "./pickRenderer";

const capable: PreviewDevice = { webgl: true, mobile: false, measuredMbps: null };

describe("Shop preview device selection", () => {
  it("skips resource timing reads when a mobile device already selects lite", () => {
    expect(pickRenderer({ webgl: true, mobile: true, get measuredMbps(): number {
      throw new Error("Resource timings were read");
    } })).toEqual({ renderer: "lite", reason: "mobile" });
  });
  it.each<[Partial<PreviewDevice>, string, string]>([
    [{ forced: "full", webgl: false }, "full", "env-override"],
    [{ forced: "lite", webgl: true }, "lite", "env-override"],
    [{ webgl: false }, "lite", "no-webgl"],
    [{ mobile: true }, "lite", "mobile"],
    [{ saveData: true }, "lite", "save-data"],
    [{ measuredMbps: 9, downlink: 20 }, "lite", "slow-connection"],
    [{ measuredMbps: 10, downlink: 1 }, "full", "connection-ok"],
    [{ downlink: 3 }, "lite", "slow-connection"],
    [{ downlink: 4 }, "full", "connection-ok"],
    [{ deviceMemory: 3 }, "lite", "low-device-memory"],
    [{ deviceMemory: 4, dpr: 2, cores: 7 }, "lite", "gpu-capability"],
    [{ dpr: 2, cores: 8 }, "full", "optimistic-default"],
    [{}, "full", "optimistic-default"],
  ])("uses %j to select %s (%s)", (device, renderer, reason) => {
    expect(pickRenderer({ ...capable, ...device })).toEqual({ renderer, reason });
  });
});
