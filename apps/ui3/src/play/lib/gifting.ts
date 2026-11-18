// /play's Gift for the shared HUD (`@hud-host`'s `gifting`): the app asks the web shell, which holds the wallet,
// over lib/giftProtocol. Outside the shell (a credentialless embed) there is no wallet, so Gift never shows.
import type { HostGifting } from '@explorer-core/hostContract'
import { inShell, onShellMessage, shellRequest } from '@explorer-core/lib/shell'
import { GIFT_EVENT, GIFT_METHODS, GIFT_PROTOCOL, isGiftEvent, parseGiftCapabilities, parsePendingGifts } from './giftProtocol'

/** How long the shell may take to take a gift: it answers as soon as it has checked the request and opened its dialog. */
const OPEN_TIMEOUT_MS = 10_000

export const gifting: HostGifting | null = inShell
  ? {
      async available() {
        try {
          return parseGiftCapabilities(await shellRequest(GIFT_METHODS.capabilities)).available
        } catch {
          return false
        }
      },
      async pending() {
        try {
          return parsePendingGifts(await shellRequest(GIFT_METHODS.pending, { v: GIFT_PROTOCOL }))
        } catch {
          return []
        }
      },
      async open(gift) {
        await shellRequest(GIFT_METHODS.open, { v: GIFT_PROTOCOL, ...gift }, OPEN_TIMEOUT_MS)
      },
      onSent(fn) {
        return onShellMessage(GIFT_EVENT, (m) => {
          if (isGiftEvent(m)) fn({ urn: m.urn, transferredAt: m.transferredAt })
        })
      }
    }
  : null
