import { useQuery } from '@tanstack/react-query'
import { PreviewRenderer } from '@dcl/schemas'
import { previewRendererOverride } from '@/data/lib/creators/config'
import { track } from '@/data/lib/creators/analytics'
import { measureDownlinkMbps } from '@/data/lib/creators/connectionSpeed'
import { FeatureFlag, getIsFeatureEnabled } from '@/data/lib/creators/featureFlags'
import { pickRenderer, type RendererDecision } from '@/data/lib/creators/pickRenderer'

async function decide(): Promise<RendererDecision> {
  // A URL override needs neither the flag nor the probe: it forces Babylon either way.
  if (previewRendererOverride) {
    return pickRenderer({ override: previewRendererOverride, unityEnabled: false, measuredMbps: null })
  }
  const unityEnabled = await getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)
  // The probe only runs when the link is what is left to decide: a flag that is off, data saver or a
  // device below the bar has already settled on Babylon, and that user should not pay for the download.
  let decision = pickRenderer({ override: null, unityEnabled, measuredMbps: null })
  let measuredMbps: number | null = null
  if (decision.renderer === PreviewRenderer.UNITY) {
    measuredMbps = await measureDownlinkMbps()
    decision = pickRenderer({ override: null, unityEnabled, measuredMbps })
  }
  try {
    track('Preview Renderer Picked', {
      renderer: decision.renderer,
      reason: decision.reason,
      measured_mbps: measuredMbps === null ? null : Math.round(measuredMbps * 10) / 10
    })
  } catch {
    // Reporting never holds the preview back.
  }
  return decision
}

/**
 * The renderer every preview mounts with, decided once per page load: `undefined` while the
 * `unity-wearable-preview` flag and the link probe are pending (callers render nothing for the preview then).
 * Reported once per page load, except under the `?unity=false` override, which is a debugging aid.
 */
export function usePreviewRenderer(): PreviewRenderer | undefined {
  return useQuery({
    queryKey: ['preview-renderer'],
    queryFn: decide,
    staleTime: Infinity,
    gcTime: Infinity,
    retry: false
  }).data?.renderer
}
