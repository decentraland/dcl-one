// What a Shop purchase request names: an item and how many of it, with the rules a line is checked by.
// The in-world cart keeps its lines by them and the /play shell protocol parses requests with them.

import { MAX_CREDIT_LINES_PER_GROUP } from "../model/settlement";

/** An item, and how many of it (a wearable or an emote can be minted more than once). */
export interface ShopLine {
  contract: string;
  itemId: string;
  quantity: number;
}

/** The most a purchase buys at once: the web Shop's cap on a Credits group (one credit pays them all). */
export const MAX_PURCHASE_UNITS = MAX_CREDIT_LINES_PER_GROUP;

/** An EVM address, in any case. */
export const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
/** A uint256 in decimal: at most 78 digits, no leading zeros. */
export const UINT = /^(0|[1-9][0-9]{0,77})$/;

/** Whether two references name the same item (contracts as given: lowercase, as parsed). */
export const sameItem = (a: { contract: string; itemId: string }, b: { contract: string; itemId: string }): boolean =>
  a.contract === b.contract && a.itemId === b.itemId;
/** How many units lines hold in all. */
export const unitsOf = (lines: readonly { quantity: number }[]): number => lines.reduce((n, l) => n + l.quantity, 0);
