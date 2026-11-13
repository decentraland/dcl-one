// The Donate panel's contract: what the host that draws it (the Sites /play web shell) shows in it, and what the
// player's actions call back. The panel is UI only; the host reads, signs and decides.

/** The least a tip may be, in MANA: the catalyst tells a creator of no smaller tip. */
export const MIN_TIP = 1

/** A tip amount as typed: whole MANA, optionally with up to 18 decimals (MANA's precision; `10` or `10.5`, not `10.`
 *  or `.5`). The panel and the host read amounts by this one rule. */
export const TIP_AMOUNT = /^\d+(\.\d{1,18})?$/

/** The creator as the panel shows them: always the address, and the profile when it could be read. */
export interface DonateCreator {
  address: string
  name?: string
  claimed?: boolean
  nameColor?: string
  face?: string
}

export type DonateView =
  | { step: 'loading' }
  | { step: 'form'; scene: string; creator: DonateCreator; balance: number; presets: number[]; usdRate: number | null; error?: string; amount?: string }
  | { step: 'sending'; amount: string; creator: DonateCreator }
  | { step: 'processing'; amount: string; creator: DonateCreator; final?: boolean }
  | { step: 'sent'; creator: DonateCreator }
  | { step: 'failed' }
  /** Another tab is sending or watching this account's tip; the panel goes on once it is done. */
  | { step: 'elsewhere' }

export interface DonateHandlers {
  onClose(): void
  /** The amount in MANA, as typed or picked ("166", "12.5"). */
  onSend?(amount: string): void
  onBuyMana?(): void
  onRetry?(): void
  onSupport?(): void
}
