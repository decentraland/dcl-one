// Confirmations before a chat link leaves the HUD or a chat coordinate moves the player.

import { useEffect, useState } from 'react'
import { DEFAULT_REALM, isNativeHud } from '@hud-host'
import { fetchPlaceAt, isEmptyPlace } from '@explorer-core/features/map/mapApi'
import { showConfirm, showToast } from '../../design'
import type { OpenExternalUrl } from '../gallery/reelActions'
import styles from './chatLinkPrompts.module.css'

/** Asks before opening `url`, naming its host first, then opens the parsed URL that was shown. The native
 *  HUD has no tabs, so the engine opens it there, and only `https:`; the web opens `http:` and `https:`. */
export async function confirmOpenLink(url: string, openExternal: OpenExternalUrl): Promise<void> {
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    showToast("This link can't be opened", { tone: 'error' })
    return
  }
  const native = isNativeHud()
  if (parsed.protocol !== 'https:' && (native || parsed.protocol !== 'http:')) {
    showToast(native ? 'Only https links can be opened' : 'Only http and https links can be opened', { tone: 'error' })
    return
  }
  const ok = await showConfirm({ title: 'Open this link?', body: <LinkTarget url={parsed} />, confirmLabel: 'Continue' })
  if (!ok) return
  if (!native) {
    window.open(parsed.href, '_blank', 'noopener,noreferrer')
    return
  }
  const opened = await openExternal(parsed.href).catch(() => false)
  if (!opened) showToast("Couldn't open this link", { tone: 'error' })
}

/** Asks before teleporting to parcel `x,y` of the current realm (`realm`, null when it has no name). */
export async function confirmTeleport(x: number, y: number, realm: string | null, teleport: (x: number, y: number) => void): Promise<void> {
  const ok = await showConfirm({ title: 'Jump to this location?', body: <TeleportTarget x={x} y={y} realm={realm} />, confirmLabel: 'Jump in' })
  if (ok) teleport(x, y)
}

/** The host on its own line, then the parsed URL, whose path and query percent-encode bidi controls. */
function LinkTarget({ url }: { url: URL }): React.JSX.Element {
  return (
    <>
      <p className={styles.host}>{url.host}</p>
      {url.protocol !== 'https:' && <p className={styles.insecure}>Not a secure (https) link</p>}
      <p className={styles.url} dir="ltr">
        {url.href}
      </p>
    </>
  )
}

/** The parcel, and the place covering it in the current realm once the places API answers. */
function TeleportTarget({ x, y, realm }: { x: number; y: number; realm: string | null }): React.JSX.Element {
  const [title, setTitle] = useState<string | null>(null)
  useEffect(() => {
    if (realm == null) return
    let live = true
    fetchPlaceAt({ x, y }, realm === DEFAULT_REALM ? undefined : realm)
      .then((place) => {
        if (live && place != null && !isEmptyPlace(place) && place.title !== '') setTitle(place.title)
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [x, y, realm])
  return (
    <p>
      {title != null && <strong>{title} </strong>}
      {x},{y}
    </p>
  )
}
