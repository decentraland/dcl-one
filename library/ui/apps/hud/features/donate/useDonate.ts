// Donate, where the host can sign a tip (`@hud-host`'s `donations`; Sites /play): whether to show the button for a
// place, opening the host's panel, and the tip emote once a tip is sent. The host resolves the creator and takes
// the amount itself; the HUD names the place only.
import { useCallback, useEffect, useState } from 'react'
import { donations } from '@hud-host'
import type { DonatePlace } from '@explorer-core/hostContract'

/** The emote the player's avatar plays once a tip is sent. */
export const TIP_EMOTE = 'urn:decentraland:off-chain:base-emotes:money'

/**
 * Whether Donate shows for `place`, and how to open it. Hidden on any failure, and while the first answer is
 * pending. With `keepWhileAsking` (the minimap, as the player walks across parcels) the last answer stays shown while
 * the next place is asked about, so it doesn't flicker; otherwise (the map, a place picked) it hides until answered.
 */
export function useDonate(
  place: DonatePlace | null,
  { keepWhileAsking = false }: { keepWhileAsking?: boolean } = {}
): { visible: boolean; open: (input?: 'pointer' | 'keyboard') => void } {
  const key = place ? JSON.stringify(place) : null
  const [shown, setShown] = useState(false)
  useEffect(() => {
    if (!donations || !key) {
      setShown(false)
      return
    }
    if (!keepWhileAsking) setShown(false)
    let alive = true
    const target = JSON.parse(key) as DonatePlace
    // The place is looked up only for a session that may tip at all.
    void (async () => {
      const visible = (await donations.available()) && (await donations.tippable(target))
      if (alive) setShown(visible)
    })()
    return () => {
      alive = false
    }
  }, [key, keepWhileAsking])
  const open = useCallback(
    (input?: 'pointer' | 'keyboard') => {
      if (!donations || !key) return
      donations.open(JSON.parse(key) as DonatePlace, input).catch((e: unknown) => console.warn('[donate] could not open', e))
    },
    [key]
  )
  return { visible: donations != null && key != null && shown, open }
}

/** Plays the tip emote each time the host says a tip was sent. */
export function useTipEmote(play: (urn: string) => void): void {
  useEffect(() => donations?.onSent(() => play(TIP_EMOTE)), [play])
}
