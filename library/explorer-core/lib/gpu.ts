// Pre-boot GPU probe. The engine (bevy / wgpu) needs a usable WebGPU adapter. A Chromium browser can
// expose `navigator.gpu` yet still have NO usable adapter — Linux without GPU drivers, hardware
// acceleration disabled, a blocklisted GPU, a VM / remote desktop, or headless. Booting there
// downloads the ~105 MB WASM and then panics ("Unable to find a GPU!"), so we probe first and show a
// friendly gate (see MobileGate `reason="gpu"`) instead of a scary post-download crash.
//
// The probe also keeps what the adapter says about itself (GPUAdapterInfo): on a Windows laptop the
// browser usually runs WebGPU on the integrated GPU, and the HUD tells the player how to switch when
// it sees one (lib/integratedGpu.ts).

const PROBE_TIMEOUT_MS = 5000

/** What WebGPU reports about the adapter it picked (GPUAdapterInfo, Chrome 121+), each field
 *  possibly empty: `vendor` is a lowercase name (`intel`, `nvidia`, `amd`, `apple`), `description`
 *  the driver's product string ("Intel(R) Iris(R) Xe Graphics"). */
export interface GpuAdapterInfo {
  vendor: string
  architecture: string
  device: string
  description: string
}

export type GpuProbe =
  /** Safe to boot the engine; `adapter` is null when the browser exposes no adapter info. */
  | { usable: true; adapter: GpuAdapterInfo | null }
  /** WebGPU absent or `requestAdapter()` resolved null: no usable GPU. */
  | { usable: false; adapter: null }

// Minimal shapes — we don't depend on @webgpu/types just for this one call.
type AdapterLike = { info?: Partial<Record<keyof GpuAdapterInfo, unknown>> }
type GpuLike = { requestAdapter(): Promise<AdapterLike | null> }

function adapterInfo(adapter: AdapterLike): GpuAdapterInfo | null {
  const info = adapter.info
  if (info == null || typeof info !== 'object') return null
  const field = (name: keyof GpuAdapterInfo): string => (typeof info[name] === 'string' ? info[name] : '')
  return { vendor: field('vendor'), architecture: field('architecture'), device: field('device'), description: field('description') }
}

/**
 * Whether the browser has a usable WebGPU adapter (safe to boot the engine), and what it reports
 * about it. Not usable when WebGPU is absent or `requestAdapter()` resolves null (no usable GPU).
 * Fails OPEN on a hung probe: a timeout counts as usable so the check never traps the user — worst
 * case we boot and hit today's panic path, never a frozen "checking" screen.
 */
export async function probeGpu(): Promise<GpuProbe> {
  const gpu = (navigator as unknown as { gpu?: GpuLike }).gpu
  if (gpu == null) return { usable: false, adapter: null }
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    const adapter = await Promise.race([
      gpu.requestAdapter(),
      new Promise<'timeout'>((resolve) => {
        timer = setTimeout(() => resolve('timeout'), PROBE_TIMEOUT_MS)
      })
    ])
    if (adapter === 'timeout') return { usable: true, adapter: null }
    if (adapter == null) return { usable: false, adapter: null }
    return { usable: true, adapter: adapterInfo(adapter) }
  } catch {
    return { usable: false, adapter: null }
  } finally {
    if (timer != null) clearTimeout(timer)
  }
}
