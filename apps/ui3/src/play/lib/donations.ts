// /play's Donate for the shared HUD (`@hud-host`'s `donations`): the app asks the web shell, which holds the wallet,
// over lib/donateProtocol. Outside the shell (a credentialless embed) there is no wallet, so Donate never shows.
import type { HostDonations } from '@explorer-core/hostContract'
import { inShell, onShellMessage, shellRequest } from '@explorer-core/lib/shell'
import { DONATE_EVENT, DONATE_METHODS, DONATE_PROTOCOL, isDonateEvent, parseDonateCapabilities } from './donateProtocol'

/** How long the shell may take to show the panel: it reads the creator, the balance and the price first. */
const OPEN_TIMEOUT_MS = 30_000

export const donations: HostDonations | null = inShell
  ? {
      async available() {
        try {
          return parseDonateCapabilities(await shellRequest(DONATE_METHODS.capabilities)).available
        } catch {
          return false
        }
      },
      async tippable(place) {
        try {
          const answer = await shellRequest<{ v?: unknown; tippable?: unknown }>(DONATE_METHODS.target, { v: DONATE_PROTOCOL, place })
          return answer?.v === DONATE_PROTOCOL && answer.tippable === true
        } catch {
          return false
        }
      },
      async open(place, input) {
        await shellRequest(DONATE_METHODS.open, { v: DONATE_PROTOCOL, place, ...(input ? { input } : {}) }, OPEN_TIMEOUT_MS)
      },
      onSent(fn) {
        return onShellMessage(DONATE_EVENT, (m) => {
          if (isDonateEvent(m)) fn()
        })
      }
    }
  : null
