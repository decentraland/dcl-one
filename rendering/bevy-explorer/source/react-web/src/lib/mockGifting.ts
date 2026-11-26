// The mock HUD's gifting (?mock=1): no wallet and no transfer. Opening a gift logs it and reports it sent a moment
// later, so the picker, its pending rule and the entry points can be checked without an engine or a shell.
import type { HostGifting, PendingGift } from '@explorer-core/hostContract'

const sent = new Set<(gift: PendingGift) => void>()
const pending: PendingGift[] = []

export const mockGifting: HostGifting = {
  available: async () => true,
  pending: async () => [...pending],
  async open(gift) {
    console.info('[mock gifting] sending', gift)
    setTimeout(() => {
      const done = { urn: gift.urn, transferredAt: gift.transferredAt }
      pending.push(done)
      for (const fn of sent) fn(done)
    }, 500)
  },
  onSent(fn) {
    sent.add(fn)
    return () => sent.delete(fn)
  }
}
