// Donate, where the host can sign a tip (`@hud-host`'s `donations`; Sites /play): whether to show the button for a
// place, opening the host's panel, and the tip emote once a tip is sent. The host resolves the creator and takes
// the amount itself; the HUD names the place only.
import { useCallback, useEffect, useState } from 'react'
import { donations } from '@hud-host'
import type { DonatePlace } from '@explorer-core/hostContract'

/** The emote the player's avatar plays once a tip is sent. */
export const TIP_EMOTE = 'urn:decentraland:off-chain:base-emotes:money'

/** Whether Donate shows for `place`, and how to open it. Hidden while either answer is pending, or on any failure. */
export function useDonate(place: DonatePlace | null): { visible: boolean; open: (input?: 'pointer' | 'keyboard') => void } {
  const key = place ? JSON.stringify(place) : null
  const [visibleFor, setVisibleFor] = useState<string | null>(null)
  useEffect(() => {
    if (!donations || !key) return
    let alive = true
    const target = JSON.parse(key) as DonatePlace
    void Promise.all([donations.available(), donations.tippable(target)]).then(([available, tippable]) => {
      if (alive) setVisibleFor(available && tippable ? key : null)
    })
    return () => {
      alive = false
    }
  }, [key])
  const open = useCallback(
    (input?: 'pointer' | 'keyboard') => {
      if (!donations || !key) return
      donations.open(JSON.parse(key) as DonatePlace, input).catch((e: unknown) => console.warn('[donate] could not open', e))
    },
    [key]
  )
  return { visible: donations != null && key != null && visibleFor === key, open }
}

/** Plays the tip emote each time the host says a tip was sent. */
export function useTipEmote(play: (urn: string) => void): void {
  useEffect(() => donations?.onSent(() => play(TIP_EMOTE)), [play])
}
