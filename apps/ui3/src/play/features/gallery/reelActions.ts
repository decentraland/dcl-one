import { showConfirm, showToast } from '../../design'
import { serviceUrl } from '../../lib/baseDomain'

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

export function reelToast(text: string, tone: 'default' | 'error' = 'default'): void {
  showToast(text, { duration: TOAST_MS, tone, key: TOAST_KEY })
}

function reelLink(id: string): string {
  return `${serviceUrl('reels')}/${id}`
}

export function shareToXUrl(id: string): string {
  return `https://x.com/intent/post?text=${encodeURIComponent(REEL_MESSAGES.shareToX)}&url=${encodeURIComponent(reelLink(id))}`
}

/** The intent url is also copied, so the post can be pasted where the browser can't open it; no toast. */
export function shareReelToX(id: string): void {
  const url = shareToXUrl(id)
  void navigator.clipboard?.writeText(url).catch(() => undefined)
  window.open(url, '_blank', 'noopener')
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
 *  instead of saving it. */
export async function downloadReel(url: string): Promise<void> {
  try {
    const res = await fetch(url)
    if (!res.ok) {
      // Release the unread error body rather than leave the connection holding it.
      await res.body?.cancel()
      throw new Error(`HTTP ${res.status}`)
    }
    const href = URL.createObjectURL(await res.blob())
    const a = document.createElement('a')
    a.href = href
    a.download = reelFileName(url)
    a.rel = 'noopener'
    document.body.appendChild(a)
    a.click()
    a.remove()
    setTimeout(() => URL.revokeObjectURL(href), 1000)
    reelToast(REEL_MESSAGES.downloaded)
  } catch (e) {
    console.warn('[gallery] download failed', e)
    reelToast(REEL_MESSAGES.failure, 'error')
  }
}

export function confirmReelDelete(): Promise<boolean> {
  return showConfirm({ title: REEL_MESSAGES.confirmDelete, cancelLabel: 'CANCEL', confirmLabel: 'DELETE' })
}
