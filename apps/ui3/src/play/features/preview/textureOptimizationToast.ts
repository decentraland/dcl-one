// The texture optimization toast: a scene preview's background texture import (dcl-scene-cli
// start converts model textures to UASTC KTX2 for a project that opts in) reported in the HUD, with
// a Reload once new optimized files are served. One toast under one key, replaced in place:
//
// - running, enabled: "Optimizing textures… done / total", until the run ends;
// - ready, enabled: the size saved and a Reload button (the scene's reload, as on a file save);
// - failed: how many textures stay PNG, or that the import failed; dismissible, no action;
// - enabled false: nothing (and a progress toast still up goes).
//
// At connect: the preview server sends a client that connects its last status first, and the
// engine marks the first status per scene within two seconds of its socket opening as `replay`
// (crates/comms/src/preview.rs). A replayed READY is ignored: the scene was just loaded from the
// files that READY announces; it only takes down a progress toast left from before a reconnect.
// A replayed RUNNING or FAILED is shown like a live one.

import { dismissToast, showToast } from '../../design'
import type { TextureOptimizationMessage } from '../../engine/protocol'

export const TEXTURE_TOAST_KEY = 'texture-optimization'

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
 *  the toast as it is (a replayed READY). */
export function textureNotice(msg: TextureOptimizationMessage): TextureNotice | null | undefined {
  if (!msg.enabled) return null
  switch (msg.state) {
    case 'running':
      return { kind: 'progress', text: `Optimizing textures… ${msg.done} / ${msg.total}` }
    case 'ready': {
      if (msg.replay) return undefined
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

// whether the toast under the key was last shown as progress (it may since have been dismissed)
let progressShown = false

/** Show (or take down) the toast for a status. `reload` reloads the scene the status is for. */
export function showTextureOptimization(msg: TextureOptimizationMessage, reload: (scene: string) => void): void {
  const notice = textureNotice(msg)
  if (notice === undefined) {
    // a replayed READY: a run ended while the socket was down, so its progress is over
    if (progressShown) dismissToast(TEXTURE_TOAST_KEY)
    progressShown = false
    return
  }
  progressShown = notice?.kind === 'progress'
  if (notice === null) {
    dismissToast(TEXTURE_TOAST_KEY)
    return
  }
  switch (notice.kind) {
    case 'progress':
      showToast(notice.text, { key: TEXTURE_TOAST_KEY, duration: null, busy: true, dismissible: true })
      return
    case 'ready':
      showToast(notice.text, {
        key: TEXTURE_TOAST_KEY,
        duration: null,
        dismissible: true,
        action: { label: 'Reload', ariaLabel: 'Reload scene to see optimized textures', onClick: () => reload(msg.sceneId) }
      })
      return
    case 'failed':
      showToast(notice.text, { key: TEXTURE_TOAST_KEY, duration: null, tone: 'error', dismissible: true })
  }
}
