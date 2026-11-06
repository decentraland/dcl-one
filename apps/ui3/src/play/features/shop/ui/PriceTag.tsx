// A card or panel price, as the web Shop's card rule (shop/discovery/cardPrice) decided it: credits with
// their mark, MANA with its glyph, or the reason there is no price.

import { formatCredits, formatCreditsFull } from '../../../../shop/checkout/lib/currency'
import CreditMark from '../../../../shop/credits/components/CreditMark'
import type { CardPrice } from '../../../../shop/discovery/cardPrice'
import { formatMana } from '../../../../shop/model/money'
import { ManaIcon } from '../../../design'

export function PriceTag({ price, size = 16 }: { price: CardPrice; size?: number }): React.JSX.Element {
  switch (price.kind) {
    case 'credits':
      return (
        <span title={`${formatCreditsFull(price.credits)} Credits`}>
          <CreditMark size={size} />
          {formatCredits(price.credits)}
        </span>
      )
    case 'mana':
      return (
        <span title={`${formatMana(price.mana, { group: true })} MANA`}>
          <ManaIcon size={size} />
          {formatMana(price.mana, { group: true, decimals: 2 })}
        </span>
      )
    case 'manaAtCheckout':
      return (
        <span>
          <ManaIcon size={size} />
          MANA
        </span>
      )
    case 'creditsUnavailable':
      return <span>Price unavailable</span>
    case 'soldOut':
      return <span>Sold out</span>
    case 'notForSale':
      return <span>Not for sale</span>
  }
}

/** Whether the buyer can buy this primary sale here, with credits (the in-world Shop's rail). */
export const buyableInWorld = (price: CardPrice): boolean => price.kind === 'credits'
