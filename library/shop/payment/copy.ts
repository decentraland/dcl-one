// The words for the payment rule. A MANA-only explanation never mentions credits, a top-up or a
// credits+MANA option; an outage says the price is unavailable and offers a retry, never another method.

import { chainLabel } from "../model/chain";
import type { MissingFact, Reason } from "../model/settlement";
import type { OptionState } from "./present";

const REASON_TEXT: Record<Reason, string> = {
  creditsFirst: "Pay with Credits.",
  manaSecond: "You can also pay with MANA from your wallet.",
  signInForCredits: "Sign in to pay with Credits.",
  secondarySale: "This is a resale: the seller is paid in MANA.",
  landOrRental: "LAND, estates and rentals are sold in MANA.",
  notDecentralandItem: "This item is not from a Decentraland collection, so it is sold in MANA.",
  bundle: "This listing sells several items together, so it is sold in MANA.",
  creditsChainUnsupported: "This item mints on Ethereum, where it is sold in MANA.",
  unsupportedCombination: "This listing is sold in MANA.",
};

export function reasonText(reason: Reason): string {
  return REASON_TEXT[reason];
}

const MISSING_TEXT: Record<MissingFact, string> = {
  manaUsdRate: "the MANA price feed is not answering",
  contractRegistry: "the contract registry is catching up",
  collectionItem: "the collection index is catching up",
  nameRegistry: "the NAME registry is catching up",
};

/** The explanation for an option the rule offers but cannot price (or, `withheld`, cannot offer) right now. */
export function outageText(method: "credits" | "mana", missing: MissingFact | null, withheld = false): string {
  if (withheld && !missing) return `Paying with ${method === "credits" ? "Credits" : "MANA"} is unavailable right now. Try again in a moment.`;
  const what = method === "credits" ? "The Credits price" : "The MANA price";
  const why = missing ? `: ${MISSING_TEXT[missing]}` : "";
  return `${what} is unavailable right now${why}. Try again in a moment.`;
}

export function manaNetworkText(chainId: number): string {
  return `Paid in MANA on ${chainLabel(chainId)}.`;
}

/** Short status wording for an option; the amounts are rendered by the caller. */
export function optionStateText(method: "credits" | "mana", state: OptionState): string | null {
  switch (state.kind) {
    case "ready":
      return null;
    case "signInRequired":
      return "Sign in to pay with Credits.";
    case "topUpRequired":
      return "Your Credits balance does not cover this. Top up to continue.";
    case "insufficientMana":
      return "Your wallet does not hold enough MANA for this purchase.";
    case "walletRequired":
      return "Connect a web3 wallet to pay with MANA.";
    case "unavailable":
      return outageText(method, state.missing, state.withheld);
  }
}
