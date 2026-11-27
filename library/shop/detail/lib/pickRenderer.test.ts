import { describe, expect, it } from "vitest";
import { pickRenderer, type PreviewDevice } from "./pickRenderer";

const capable: PreviewDevice = { webgl: true, mobile: false, measuredMbps: null };

describe("Shop preview device selection", () => {
  it.each<[Partial<PreviewDevice>, string]>([
    [{ webgl: false }, "no-webgl"],
    [{ forced: "full" }, "env-override"],
  ])("skips the viewport measurement when %j settles the decision", (device, reason) => {
    const result = pickRenderer({
      ...capable,
      ...device,
      get mobile(): boolean {
        throw new Error("The viewport was measured");
      },
    });
    expect(result.reason).toBe(reason);
  });

  it.each<[Partial<PreviewDevice>, string, string]>([
    [{ forced: "full", webgl: false }, "full", "env-override"],
    [{ forced: "lite", webgl: true }, "lite", "env-override"],
    [{ webgl: false }, "lite", "no-webgl"],
    [{ mobile: true }, "lite", "mobile"],
    [{ saveData: true }, "lite", "save-data"],
    [{ measuredMbps: 9 }, "lite", "slow-connection"],
    [{ measuredMbps: 10 }, "full", "connection-ok"],
    [{ deviceMemory: 3 }, "lite", "low-device-memory"],
    [{ deviceMemory: 4, dpr: 2, cores: 7 }, "lite", "gpu-capability"],
    [{ dpr: 2, cores: 8 }, "full", "optimistic-default"],
    [{}, "full", "optimistic-default"],
  ])("uses %j to select %s (%s)", (device, renderer, reason) => {
    expect(pickRenderer({ ...capable, ...device })).toEqual({ renderer, reason });
  });
});
