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
 * only while `dapps-shop-credits-mana-split` is on (`split`), the same switch the web Shop and the credits node read: the
 * node refuses a Credits part while it is off, whatever a page shows.
 */
export function seesSplit({ flag, split, wallet }: { flag: boolean; split: boolean; wallet: ShopWalletKind | null }): boolean {
  return split && seesMana({ flag, wallet })
}
