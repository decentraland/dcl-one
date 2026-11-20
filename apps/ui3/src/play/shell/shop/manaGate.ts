// The one rule for whether a buyer is shown paying in MANA in the in-world Shop: the MANA equivalent under a price, the
// MANA row when choosing how to pay, a cart paid in MANA. Every MANA feature asks this, the shell's purchase directly and
// the HUD through capabilities' `mana`, so showing MANA to more buyers (every wallet, say) is a change here alone.

import type { ShopWalletKind } from '@dcl/one-ui/apps/shop/shopProtocol'

/**
 * Whether a buyer whose wallet here is `wallet` sees MANA, with the `shop-mana-in-world` switch read as `flag`. Today:
 * the switch on, and a self-custody (web3) wallet, the only kind here that holds MANA to pay with.
 */
export function seesMana({ flag, wallet }: { flag: boolean; wallet: ShopWalletKind | null }): boolean {
  return flag && wallet === 'web3'
}

/**
 * Paying one item with Credits and MANA together (the balance pays part, MANA the rest): only to a buyer shown MANA, and
 * off until the credits node authorizes part of a price. `setSplitInWorld` turns it on (tests, then the node's release).
 */
let splitOn = false
export function seesSplit(who: { flag: boolean; wallet: ShopWalletKind | null }): boolean {
  return splitOn && seesMana(who)
}
export function setSplitInWorld(on: boolean): () => void {
  const previous = splitOn
  splitOn = on
  return () => {
    splitOn = previous
  }
}
