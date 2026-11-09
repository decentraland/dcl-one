// Windows laptops usually carry two GPUs, and Chromium runs WebGPU on the one the rest of the
// browser uses — normally the integrated one — ignoring `powerPreference: "high-performance"`.
// Nothing on this side can pick the other GPU; the player switches the browser over in Windows'
// graphics settings or with the browser's own flag, then relaunches it. This decides when the HUD
// shows that guide (features/gate/IntegratedGpuGuide.tsx): on Windows, with an Intel adapter, the
// one case where the integrated GPU is near-certain. An AMD APU cannot be told from a discrete AMD
// card by what WebGPU reports, so it gets no guide.

import type { GpuAdapterInfo } from './gpu'

/** Intel Arc is Intel's discrete line: switching would change nothing. */
const INTEL_DISCRETE = /\bArc\b/i

export function integratedGpuOnWindows(adapter: GpuAdapterInfo | null, ua: string = navigator.userAgent): boolean {
  if (adapter == null || !/Windows NT/.test(ua)) return false
  const intel = adapter.vendor.toLowerCase() === 'intel' || /\bIntel\b/i.test(adapter.description)
  return intel && !INTEL_DISCRETE.test(adapter.description)
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
