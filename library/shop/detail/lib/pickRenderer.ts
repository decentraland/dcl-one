// from: shop@ae3083ac:app/src/lib/pickRenderer.ts
// The measured speed is the preview probe download. Page resource timings include latency on small chunks.
export type PreviewRenderer = "full" | "lite";

export const FULL_MIN_MBPS = 10;
export const FULL_MIN_DEVICE_MEMORY = 4;
export const FULL_MIN_DPR = 2;
export const FULL_HIDPI_MIN_HW_CONCURRENCY = 8;

export type RendererReason =
  | "mobile"
  | "save-data"
  | "slow-connection"
  | "low-device-memory"
  | "gpu-capability"
  | "connection-ok"
  | "optimistic-default"
  | "env-override"
  | "no-webgl";

export type RendererDecision = { renderer: PreviewRenderer; reason: RendererReason };

export type PreviewDevice = {
  forced?: string;
  webgl: boolean;
  mobile: boolean;
  saveData?: boolean;
  measuredMbps: number | null;
  deviceMemory?: number;
  dpr?: number;
  cores?: number;
};
export function pickRenderer(device: PreviewDevice): RendererDecision {
  const { forced } = device;
  const lite = (reason: RendererReason): RendererDecision => ({ renderer: "lite", reason });
  if (forced === "lite") return lite("env-override");
  if (forced === "full") return { renderer: "full", reason: "env-override" };
  if (!device.webgl) return lite("no-webgl");
  if (device.mobile) return lite("mobile");
  if (device.saveData) return lite("save-data");
  const { measuredMbps: measured, deviceMemory, dpr, cores } = device;
  if (measured !== null && measured < FULL_MIN_MBPS) return lite("slow-connection");
  if (deviceMemory !== undefined && deviceMemory < FULL_MIN_DEVICE_MEMORY) return lite("low-device-memory");
  if (dpr !== undefined && dpr >= FULL_MIN_DPR && cores !== undefined && cores < FULL_HIDPI_MIN_HW_CONCURRENCY) return lite("gpu-capability");
  return { renderer: "full", reason: measured !== null ? "connection-ok" : "optimistic-default" };
}
