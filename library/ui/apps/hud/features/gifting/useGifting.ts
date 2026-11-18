// Gift, where the host can sign a transfer (`@hud-host`'s `gifting`; Sites /play): whether to offer it, and the
// tokens already sent as gifts that the owned catalog may still list. The host checks and signs each gift itself.
import { useEffect, useState } from 'react'
import { gifting } from '@hud-host'
import type { PendingGift } from '@explorer-core/hostContract'
import { useOptionalSession } from '../session/SessionContext'

/**
 * Whether Gift is offered to this session: a host that can sign, and its yes for this sign-in (the session's phase
 * and account), asked as the surface opens. Hidden on any failure, and while the answer is pending.
 */
export function useGiftingAvailable(): boolean {
  const session = useOptionalSession()
  const signIn = `${session?.phase ?? ''}|${session?.login.account ?? ''}`
  const [available, setAvailable] = useState(false)
  useEffect(() => {
    setAvailable(false)
    if (!gifting) return
    let alive = true
    void gifting.available().then(
      (yes) => alive && setAvailable(yes),
      () => {}
    )
    return () => {
      alive = false
    }
  }, [signIn])
  return gifting != null && available
}

/**
 * The tokens sent as gifts that may not have left the account yet: read from the host as the picker opens, plus each
 * one the host sends while it is open. Gifts the host reports while the read is in flight are kept.
 */
export function usePendingGifts(): readonly PendingGift[] {
  const [pending, setPending] = useState<readonly PendingGift[]>([])
  useEffect(() => {
    if (!gifting) return
    let alive = true
    const add = (gift: PendingGift): void =>
      setPending((list) => (list.some((p) => p.urn === gift.urn && p.transferredAt === gift.transferredAt) ? list : [...list, gift]))
    const stop = gifting.onSent(add)
    void gifting.pending().then(
      (known) => alive && known.forEach(add),
      () => {}
    )
    return () => {
      alive = false
      stop()
    }
  }, [])
  return pending
}
