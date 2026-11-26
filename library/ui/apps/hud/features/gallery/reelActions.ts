import { createElement, type ReactNode } from 'react'
import { showConfirm, showToast } from '../../design'
import { DownloadedToast } from './DownloadedToast'
import checkIcon from '../../assets/lobby/checkmark-icn-thin.png'
import warningIcon from '../../assets/startup/warning.png'
import { isNativeHud, serviceUrl } from '@hud-host'
import { cefDownloadOutcome } from '@explorer-core/lib/cefNativeBridge'
import { worldOfRealm } from '@explorer-core/features/lobby/goto'
import type { MapState, TravelOutcome } from '../session/useEngineSession'
import { MaskIcon } from '@dcl/one-ui'

export const REEL_MESSAGES = {
  shareToX: 'Happening right now in @decentraland.\n\nCome hang out 👋\n\n',
  deleted: 'Photo successfully deleted',
  updated: 'Photo successfully updated',
  downloaded: 'Photo successfully downloaded',
  linkCopied: 'Link copied!',
  failure: 'There was an error while trying to process your request. Please try again!',
  confirmDelete: 'Are you sure you want to permanently delete this photo?'
} as const

const TOAST_MS = 3000
const TOAST_KEY = 'camera-reel'
const NATIVE_DOWNLOAD_TIMEOUT_MS = 30_000

export type OpenExternalUrl = (url: string) => Promise<boolean>

export function reelToast(content: ReactNode, tone: 'default' | 'error' = 'default'): void {
  const icon =
    tone === 'error' ? createElement(MaskIcon, { src: warningIcon, size: 24 }) : createElement(MaskIcon, { src: checkIcon, width: 24, height: 14 })
  showToast(content, { duration: TOAST_MS, tone, key: TOAST_KEY, icon })
}

/** A web page outside the HUD: the engine opens it on native, where the HUD has no tabs. */
export function openExternal(url: string, openExternalUrl?: OpenExternalUrl): void {
  if (isNativeHud() && openExternalUrl != null) void openExternalUrl(url)
  else window.open(url, '_blank', 'noopener')
}

function reelLink(id: string): string {
  return `${serviceUrl('reels')}/${id}`
}

export function shareToXUrl(id: string): string {
  return `https://x.com/intent/post?text=${encodeURIComponent(REEL_MESSAGES.shareToX)}&url=${encodeURIComponent(reelLink(id))}`
}

/** The intent url is also copied, so the post can be pasted where the browser can't open it; no
 *  toast. The native HUD has no browser tabs of its own, so there the engine opens the url. */
export function shareReelToX(id: string, openExternalUrl?: OpenExternalUrl): void {
  const url = shareToXUrl(id)
  void navigator.clipboard?.writeText(url).catch(() => undefined)
  openExternal(url, openExternalUrl)
}

export function copyReelLink(id: string): void {
  const write = navigator.clipboard?.writeText(reelLink(id))
  if (write == null) {
    reelToast(REEL_MESSAGES.failure, 'error')
    return
  }
  write.then(
    () => reelToast(REEL_MESSAGES.linkCopied),
    () => reelToast(REEL_MESSAGES.failure, 'error')
  )
}

export function reelFileName(url: string): string {
  try {
    const name = new URL(url).pathname.split('/').pop()
    return name != null && name !== '' ? decodeURIComponent(name) : 'photo.jpg'
  } catch {
    return 'photo.jpg'
  }
}

/** Fetched first: a cross-origin link's `download` is ignored, so the browser would open the image
 *  instead of saving it. On native the engine saves the file, so the toast waits for its report.
 *  Resolves whether it was saved. */
export async function downloadReel(url: string): Promise<boolean> {
  try {
    const res = await fetch(url)
    if (!res.ok) {
      // Release the unread error body rather than leave the connection holding it.
      await res.body?.cancel()
      throw new Error(`HTTP ${res.status}`)
    }
    const href = URL.createObjectURL(await res.blob())
    const saved = cefDownloadOutcome(href, NATIVE_DOWNLOAD_TIMEOUT_MS)
    const a = document.createElement('a')
    a.href = href
    a.download = reelFileName(url)
    a.rel = 'noopener'
    document.body.appendChild(a)
    a.click()
    a.remove()
    if (saved == null) {
      setTimeout(() => URL.revokeObjectURL(href), 1000)
      reelToast(REEL_MESSAGES.downloaded)
      return true
    }
    const { ok, dir } = await saved
    URL.revokeObjectURL(href)
    if (!ok) throw new Error('the engine did not save the file')
    reelToast(dir == null ? REEL_MESSAGES.downloaded : createElement(DownloadedToast, { message: REEL_MESSAGES.downloaded, dir }))
    return true
  } catch (e) {
    console.warn('[gallery] download failed', e)
    reelToast(REEL_MESSAGES.failure, 'error')
    return false
  }
}

export function confirmReelDelete(): Promise<boolean> {
  return showConfirm({ title: REEL_MESSAGES.confirmDelete, cancelLabel: 'CANCEL', confirmLabel: 'DELETE' })
}

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

/** `MMMM dd, yyyy` in local time, e.g. "March 08, 2025". */
export function photoDate(ms: number): string {
  if (!ms) return ''
  const d = new Date(ms)
  return `${MONTHS[d.getMonth()]} ${String(d.getDate()).padStart(2, '0')}, ${d.getFullYear()}`
}

/** Where a photo was taken: a world's realm (`name.dcl.eth`) or Genesis City, and the parcel. */
export interface PhotoPlace {
  realm?: string
  x: number
  y: number
}

export function photoDestination(place: PhotoPlace): { kind: 'world'; realm: string } | { kind: 'parcel'; x: number; y: number } {
  return place.realm != null && worldOfRealm(place.realm) != null ? { kind: 'world', realm: place.realm } : { kind: 'parcel', x: place.x, y: place.y }
}

/** A world's photo changes realm to the world; any other teleports to its Genesis City parcel,
 *  leaving a world first. */
export function travelToPhoto(place: PhotoPlace, map: Pick<MapState, 'teleportToPlace' | 'changeRealm'>): TravelOutcome {
  const dest = photoDestination(place)
  return dest.kind === 'world' ? map.changeRealm(dest.realm) : map.teleportToPlace(dest.x, dest.y)
}
