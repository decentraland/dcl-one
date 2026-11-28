// The Auto Graphics message: each time the engine settles on lower graphics for the session, one
// toast says what changed and that the saved settings did not. It is shown only while the player
// is in the world with the HUD's chrome up; a notice arriving at any other time (a menu page, photo
// mode, the editor, the lobby, a hidden HUD) is dropped, not held for later, since the picture it
// describes has changed again by then. A later notice replaces the toast in place.

import { useEffect } from 'react'
import type { AutoGraphicsNotice } from '@explorer-core/engine/generated'
import { dismissToast, showToast } from '../../design'

export const AUTO_GRAPHICS_TOAST_KEY = 'autoGraphics'
export const AUTO_GRAPHICS_TITLE = 'Graphics lowered to keep things smooth'
export const AUTO_GRAPHICS_REASSURANCE = "Your saved settings haven't changed. You can turn off Auto Graphics in Settings."
/** How long the toast stays when the player does nothing. */
export const AUTO_GRAPHICS_TOAST_MS = 15_000
/** The Settings tab that holds the Auto Graphics setting. */
export const AUTO_GRAPHICS_SETTINGS_TAB = 'Performance'

/** What changed, such as "Shadows off · Resolution 70% · Bloom off": only the parts that apply. */
export function autoGraphicsSummary(notice: AutoGraphicsNotice): string {
  const parts: string[] = []
  if (notice.shadowsOff) parts.push('Shadows off')
  if (notice.renderScalePercent != null) parts.push(`Resolution ${notice.renderScalePercent}%`)
  if (notice.bloomOff) parts.push('Bloom off')
  return parts.join(' · ')
}

/** Show the toast for a notice; `openSettings` opens Settings on the Performance tab. */
export function showAutoGraphicsToast(notice: AutoGraphicsNotice, openSettings: () => void): void {
  const summary = autoGraphicsSummary(notice)
  showToast(
    <>
      <strong>{AUTO_GRAPHICS_TITLE}</strong>
      {summary !== '' && (
        <>
          <br />
          {summary}
        </>
      )}
      <br />
      {AUTO_GRAPHICS_REASSURANCE}
    </>,
    {
      key: AUTO_GRAPHICS_TOAST_KEY,
      duration: AUTO_GRAPHICS_TOAST_MS,
      dismissible: true,
      action: { label: 'Settings', ariaLabel: 'Open graphics settings', onClick: openSettings }
    }
  )
}

interface NoticeState {
  notice: (AutoGraphicsNotice & { id: number }) | null
  dismiss: () => void
}

/** Turn the session's notice into the toast while `active`, and take the notice either way: one that
 *  arrives while not active is dropped. The toast goes when `active` turns false. */
export function useAutoGraphicsToast({ notice, dismiss }: NoticeState, active: boolean, openSettings: (tab: string) => void): void {
  useEffect(() => {
    if (!active) dismissToast(AUTO_GRAPHICS_TOAST_KEY)
    if (notice == null) return
    if (active) showAutoGraphicsToast(notice, () => openSettings(AUTO_GRAPHICS_SETTINGS_TAB))
    dismiss()
  }, [notice, active, dismiss, openSettings])
}
