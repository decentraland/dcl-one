// Windows laptops usually carry two GPUs, and Chromium runs WebGPU on the one the rest of the
// browser uses — normally the integrated one — ignoring `powerPreference: "high-performance"`.
// Nothing on this side can pick the other GPU; the player switches the browser over in Windows'
// graphics settings or with the browser's own flag, then relaunches it. This decides when the HUD
// shows that guide (features/gate/IntegratedGpuGuide.tsx): on Windows, with an Intel adapter, the
// one case where the integrated GPU is near-certain. An AMD APU cannot be told from a discrete AMD
// card by what WebGPU reports, so it gets no guide.

import type { GpuAdapterInfo } from './gpu'

/** Intel's discrete line, Arc: switching would change nothing. Chrome fills `architecture` by
 *  default and `description` only with WebGPU developer features on, so the architecture is the
 *  reliable tell: Dawn's "Gen 12 HP" (Arc A, Alchemist) and "Xe 2 HPG" (Arc B, Battlemage),
 *  reported as `gen-12hp` / `xe-2hpg`. The description's model number backs it up; the bare
 *  "Arc" brand does not, Core Ultra iGPUs read "Intel(R) Arc(TM) Graphics". */
const INTEL_DISCRETE_ARCHITECTURES = new Set(['gen12hp', 'xe2hpg'])
const INTEL_DISCRETE_MODEL = /\bArc(?:\(TM\))?\s+[AB]\d{3}\b/i

export function integratedGpuOnWindows(adapter: GpuAdapterInfo | null, ua: string = navigator.userAgent): boolean {
  if (adapter == null || !/Windows NT/.test(ua)) return false
  const intel = adapter.vendor.toLowerCase() === 'intel' || /\bIntel\b/i.test(adapter.description)
  if (!intel) return false
  const architecture = adapter.architecture.toLowerCase().replace(/[^a-z0-9]/g, '')
  return !INTEL_DISCRETE_ARCHITECTURES.has(architecture) && !INTEL_DISCRETE_MODEL.test(adapter.description)
}

export interface ChromiumBrowser {
  name: string
  /** The flags page for "Force high performance GPU", behind the browser's own scheme. Every
   *  Chromium build also accepts `chrome://`, so the default is never wrong, only less familiar. */
  flagsUrl: string
}

/** The Chromium browser the UA names: Edge and Opera mark theirs, the rest (Chrome, Brave, which
 *  hides itself) read as Chrome. */
export function chromiumBrowser(ua: string = navigator.userAgent): ChromiumBrowser {
  const [name, scheme] = /\bEdg\//.test(ua) ? ['Edge', 'edge'] : /\bOPR\//.test(ua) ? ['Opera', 'opera'] : ['Chrome', 'chrome']
  return { name, flagsUrl: `${scheme}://flags/#force-high-performance-gpu` }
}
