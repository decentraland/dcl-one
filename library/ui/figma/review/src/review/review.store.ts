import { platforms, type Platform } from './navigation'
import { createStore } from '@tanstack/react-store'
import { breakpoints } from '../generated/tokens/tokens'

type CanvasMode = 'source' | 'full' | 'custom' | keyof typeof breakpoints
function savedCanvas(): { canvasMode: CanvasMode; canvasWidth: number } {
  try {
    const value = JSON.parse(localStorage.getItem('dcl-review-canvas') ?? 'null')
    if (
      value &&
      ['source', 'full', 'custom', ...Object.keys(breakpoints)].includes(value.canvasMode) &&
      Number.isFinite(value.canvasWidth)
    )
      return {
        canvasMode: value.canvasMode,
        canvasWidth: Math.min(7680, Math.max(160, value.canvasWidth))
      }
  } catch {
    /* Storage may be unavailable in private browser sessions. */
  }
  return { canvasMode: 'source', canvasWidth: 640 }
}
const reviewStore = createStore({
  ...savedCanvas(),
  navigationSearch: '',
  platform: (platforms.includes(new URLSearchParams(location.search).get('platform') as Platform)
    ? new URLSearchParams(location.search).get('platform')
    : 'all') as Platform
})

export { reviewStore }
export type { CanvasMode }
