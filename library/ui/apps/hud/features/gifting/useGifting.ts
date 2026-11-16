// Gift, where the host can sign a transfer (`@hud-host`'s `gifting`; Sites /play): whether to offer it, and the
// tokens already sent as gifts that the owned catalog may still list. The host checks and signs each gift itself.
import { useEffect, useState, useSyncExternalStore } from 'react'
import { gifting } from '@hud-host'
import type { PendingGift } from '@explorer-core/hostContract'
import { useOptionalSession } from '../session/SessionContext'

/**
 * Whether Gift is offered to this session: a host that can sign, and its yes for this sign-in (the session's phase
 * and account). Hidden on any failure, and while the answer is pending.
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

let pending: PendingGift[] = []
const listeners = new Set<() => void>()
const emit = (): void => listeners.forEach((l) => l())

function add(gift: PendingGift): void {
  if (pending.some((p) => p.urn === gift.urn && p.transferredAt === gift.transferredAt)) return
  pending = [...pending, gift]
  emit()
}

/** Reads the host's pending gifts again (the picker opened): the host keeps them per account, and stores each one
 *  before it says it was sent. */
export async function refreshPendingGifts(): Promise<void> {
  if (!gifting) return
  pending = await gifting.pending().catch(() => [])
  emit()
}

let unsubscribe: (() => void) | null = null
function listen(): void {
  if (unsubscribe != null || !gifting) return
  unsubscribe = gifting.onSent(add)
}

/** The tokens sent as gifts this session knows of, kept current. */
export function usePendingGifts(): readonly PendingGift[] {
  useEffect(listen, [])
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => pending
  )
}
