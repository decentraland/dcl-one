// The texture optimization toast: a scene preview's background texture import (dcl-scene-cli
// start converts model textures to UASTC KTX2 for a project that opts in) reported in the HUD, with
// a Reload once new optimized files are served. One toast per scene, under its own key, replaced in
// place:
//
// - running, enabled: "Optimizing textures… done / total", until the run ends (no Dismiss: the next
//   update, every 500 ms, would bring it back);
// - ready, enabled: the size saved and a Reload button (the scene's reload, as on a file save);
// - failed: how many textures stay PNG, or that the import failed; dismissible, no action;
// - enabled false: nothing (and a toast still up goes).
//
// At connect the preview server sends its last status for each scene again, marked `replay`. A
// replayed READY offers Reload only when this HUD showed that scene's progress: the run ended while
// the socket was down. Otherwise the scene was loaded from the files it announces, and the toast is
// left as it is. A replayed RUNNING or FAILED is shown like a live one.

import { dismissToast, showToast } from '../../design'
import type { TextureOptimizationMessage } from '@explorer-core/engine/protocol'

export const TEXTURE_TOAST_KEY = 'texture-optimization'

/** The toast key of one scene's texture optimization. */
export function textureToastKey(sceneId: string): string {
  return `${TEXTURE_TOAST_KEY}:${sceneId}`
}

const UNITS = ['KB', 'MB', 'GB']

/** 950 B, 12 KB, 4.2 MB: one decimal below 10 of a unit, 1024 bytes to a KB. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes || 0))} B`
  let value = bytes
  let unit = ''
  for (const u of UNITS) {
    value /= 1024
    unit = u
    if (value < 1024) break
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${unit}`
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

export type TextureNotice =
  | { kind: 'progress'; text: string }
  | { kind: 'ready'; text: string }
  | { kind: 'failed'; text: string }

/** What the HUD shows for a status: a notice, `null` to take the toast down, `undefined` to leave
 *  the toast as it is (a replayed READY for a scene whose progress this HUD did not show). */
export function textureNotice(msg: TextureOptimizationMessage, sawProgress = false): TextureNotice | null | undefined {
  if (!msg.enabled) return null
  switch (msg.state) {
    case 'running':
      return { kind: 'progress', text: `Optimizing textures… ${msg.done} / ${msg.total}` }
    case 'ready': {
      if (msg.replay && !sawProgress) return undefined
      const saved = msg.bytesBefore > 0 ? ` · ${formatBytes(msg.bytesBefore)} → ${formatBytes(msg.bytesAfter)}` : ''
      const kept = msg.failed > 0 ? ` (${plural(msg.failed, 'texture stays', 'textures stay')} PNG)` : ''
      return { kind: 'ready', text: `Textures optimized${saved}${kept} — Reload to see them` }
    }
    case 'failed':
      return {
        kind: 'failed',
        text:
          msg.failed > 0
            ? `${plural(msg.failed, "texture couldn't", "textures couldn't")} be optimized and stay as PNG`
            : "Textures couldn't be optimized; the preview keeps serving the last optimized files"
      }
  }
}

// the scenes whose toast was last shown as progress
const progressShown = new Set<string>()

/** Show (or take down) the toast for a status. `reload` reloads the scene the status is for. */
export function showTextureOptimization(msg: TextureOptimizationMessage, reload: (scene: string) => void): void {
  const key = textureToastKey(msg.sceneId)
  const notice = textureNotice(msg, progressShown.has(msg.sceneId))
  if (notice === undefined) return
  if (notice?.kind === 'progress') progressShown.add(msg.sceneId)
  else progressShown.delete(msg.sceneId)
  if (notice === null) {
    dismissToast(key)
    return
  }
  switch (notice.kind) {
    case 'progress':
      showToast(notice.text, { key, duration: null, busy: true })
      return
    case 'ready':
      showToast(notice.text, {
        key,
        duration: null,
        dismissible: true,
        action: { label: 'Reload', ariaLabel: 'Reload scene to see optimized textures', onClick: () => reload(msg.sceneId) }
      })
      return
    case 'failed':
      showToast(notice.text, { key, duration: null, tone: 'error', dismissible: true })
  }
}
