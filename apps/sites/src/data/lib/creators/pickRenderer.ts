// Chooses the preview renderer, ported from shop's lib/pickRenderer WITHOUT its mobile short-circuit:
// the editor keeps Unity on phones too. Decided once per page load by `usePreviewRenderer`.
import { PreviewRenderer } from '@dcl/schemas'

// Benchmark: Unity ~2.8s at ~300Mbps vs ~15.7s at ~4Mbps — require a comfortable link for the big bundle.
export const UNITY_MIN_MBPS = 10
export const UNITY_MIN_DEVICE_MEMORY = 4 // GB
// A hi-DPI screen paired with a modest core count is a proxy for a mid-tier GPU that would struggle
// to drive full-DPR Unity.
export const UNITY_MIN_DPR = 2
export const UNITY_HIDPI_MIN_HW_CONCURRENCY = 8

export type RendererReason =
  | 'url-override'
  | 'flag-off'
  | 'save-data'
  | 'slow-connection'
  | 'low-device-memory'
  | 'gpu-capability'
  | 'connection-ok'
  | 'optimistic-default'

export type RendererDecision = { renderer: PreviewRenderer; reason: RendererReason }

export type RendererInputs = {
  /** `?unity=false` resolved by `~/config`. */
  override: 'babylon' | null
  /** The `unity-wearable-preview` flag; unknown counts as off. */
  unityEnabled: boolean
  /** The link as `lib/connectionSpeed` measured it; `null` when it could not. */
  measuredMbps: number | null
}

type NavigatorConnection = { saveData?: boolean }

function connection(): NavigatorConnection | undefined {
  if (typeof navigator === 'undefined') return undefined
  return (navigator as unknown as { connection?: NavigatorConnection }).connection
}

function deviceMemory(): number | undefined {
  if (typeof navigator === 'undefined') return undefined
  return (navigator as unknown as { deviceMemory?: number }).deviceMemory
}

function hardwareConcurrency(): number | undefined {
  if (typeof navigator === 'undefined') return undefined
  return (navigator as unknown as { hardwareConcurrency?: number }).hardwareConcurrency
}

function devicePixelRatio(): number | undefined {
  if (typeof window === 'undefined') return undefined
  return typeof window.devicePixelRatio === 'number' ? window.devicePixelRatio : undefined
}

/** Unity when allowed and the device/connection look up to it; unknown signals stay optimistic. */
export function pickRenderer({ override, unityEnabled, measuredMbps }: RendererInputs): RendererDecision {
  const babylon = (reason: RendererReason): RendererDecision => ({ renderer: PreviewRenderer.BABYLON, reason })

  if (override === 'babylon') return babylon('url-override')
  if (!unityEnabled) return babylon('flag-off')

  if (connection()?.saveData) return babylon('save-data')
  if (measuredMbps !== null && measuredMbps < UNITY_MIN_MBPS) return babylon('slow-connection')

  const memory = deviceMemory()
  if (memory !== undefined && memory < UNITY_MIN_DEVICE_MEMORY) return babylon('low-device-memory')

  const dpr = devicePixelRatio()
  const cores = hardwareConcurrency()
  if (dpr !== undefined && dpr >= UNITY_MIN_DPR && cores !== undefined && cores < UNITY_HIDPI_MIN_HW_CONCURRENCY) {
    return babylon('gpu-capability')
  }

  return { renderer: PreviewRenderer.UNITY, reason: measuredMbps !== null ? 'connection-ok' : 'optimistic-default' }
}
