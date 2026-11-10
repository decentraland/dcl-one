// The purchase offer the NAME registration screens render. The server is the authority: when the host
// wires the Shop's quote route (`serverNameOffer`), its PurchaseOffer is used as it comes. Until that
// route is mounted, `interimNameOffer` assembles the same shape from what the server and the chain
// already answer: the registrar's `available(name)` (a taken NAME is a resale, never a registration),
// the rule mirror of payment/rule.ts for kind `name` + sale `primaryNameRegistration`, the controller's
// fixed 100 MANA and the credits quote of `/shop-api/v1/credit-quotes`. It can only ever present the
// registration rule; every entry (the credit authorization with the NAME, the credit route, the
// registrar) re-checks it, and a credits quote the server cannot give is shown as unavailable, never
// replaced by MANA.

import { missingOfferRoute, type OfferSource } from "../detail/offers";
import { type ShopClient } from "@shop/api/shop";
import { evmAddress } from "@shop/model/address";
import { nameAsset, parseNameLabel, type NameLabel } from "@shop/model/asset";
import { CHAIN, type ChainId } from "@shop/model/chain";
import { manaWei } from "@shop/model/money";
import type { PurchaseOffer } from "@shop/model/purchase";
import type { MissingFact, PaymentMethod, Quote, Session, SettlementGroupKey } from "@shop/model/settlement";
import { decodePurchaseOffer } from "@shop/model/wire";
import { present, restrictions } from "@shop/payment/rule";
import { landChainId } from "../land/chain";
import { landNetwork } from "../land/contracts";
import { rpcReader, type ChainReader } from "../land/evm";
import { quoteNameCredits } from "./api";
import { checkNameAvailability, creditsChainFor, NAME_PRICE_WEI } from "./names";
import { shopClient } from "../api/host";

export type NameOfferSource = (label: NameLabel, session: Session, signal: AbortSignal) => Promise<PurchaseOffer>;

/** The registrar says the NAME exists: it can only be bought as a resale, in MANA. */
export class NameAlreadyRegisteredError extends Error {
  override name = "NameAlreadyRegisteredError";
  constructor(readonly label: string) {
    super(`${label}.dcl.eth is already registered.`);
  }
}

/** The server's quote route: `fetch` answers the serde PurchaseOffer, decoded strictly. */
export function serverNameOffer(fetch: (label: NameLabel, signal: AbortSignal) => Promise<unknown>): NameOfferSource {
  return async (label, _session, signal) => {
    const offer = decodePurchaseOffer(await fetch(label, signal));
    if (!offer) throw new Error("The server's NAME offer did not decode");
    return offer;
  };
}

/**
 * The Shop's one offer seam (detail/offers `ShopOfferProvider`) asked for a NAME registration: the query
 * names the registrar token and the label. A null answer is a NAME that cannot be registered (it
 * exists). Null when no host installed a source, so the caller falls back to `interimNameOffer`.
 */
export function seamNameOffer(source: OfferSource, chainId: ChainId = landChainId()): NameOfferSource | null {
  if (source === missingOfferRoute) return null;
  return async (label, session, signal) => {
    const network = landNetwork(chainId);
    const asset = nameAsset({ chainId, contract: network.nameRegistrar }, label);
    const key = `name:${label.toLowerCase()}`;
    const answers = await source([{ key, asset, listing: { name: label } }], session, signal);
    const offer = answers[key];
    if (!offer) throw new NameAlreadyRegisteredError(label);
    return offer;
  };
}

/** The Across NAME executor on Polygon (docs/shop/CONTRACTS.md): the credits call's target. */
export const NAME_CREDITS_EXECUTOR: Record<number, string> = {
  [CHAIN.POLYGON]: "0xc9a9c416fbfd424d09e3f905142799abb10c2fa7",
};

export function interimNameOffer(opts: { client?: ShopClient; reader?: ChainReader; chainId?: ChainId; now?: () => number } = {}): NameOfferSource {
  return async (label, session, signal) => {
    const chainId = opts.chainId ?? landChainId();
    const network = landNetwork(chainId);
    const creditsChain = creditsChainFor(chainId);
    const client = opts.client ?? shopClient();
    const [availability, credits] = await Promise.all([
      checkNameAvailability(label, { signal, reader: opts.reader ?? rpcReader(chainId), chainId }),
      quoteNameCredits(client, signal),
    ]);
    if (availability === "taken") throw new NameAlreadyRegisteredError(label);
    const registrar = { chainId, contract: network.nameRegistrar };
    const asset = nameAsset(registrar, label);
    const eligibility = present(restrictions("name", "primaryNameRegistration", chainId, false), session, chainId);
    const quotes: Quote[] = [];
    const unavailable: { method: PaymentMethod; missing: MissingFact }[] = [];
    const settlement: { method: PaymentMethod; key: SettlementGroupKey }[] = [];
    for (const method of eligibility.allowed) {
      if (method === "credits") {
        if ("quoted" in credits) quotes.push({ method, quoted: credits.quoted });
        else unavailable.push({ method, missing: credits.missing });
        const executor = NAME_CREDITS_EXECUTOR[creditsChain];
        if (executor) settlement.push({ method, key: { method, chainId: creditsChain, contract: evmAddress(executor), call: { kind: "one", asset } } });
      } else {
        quotes.push({ method, amount: manaWei(NAME_PRICE_WEI) });
        settlement.push({ method, key: { method, chainId, contract: network.nameController, call: { kind: "one", asset } } });
      }
    }
    return {
      asset,
      kind: "name",
      sale: "primaryNameRegistration",
      bundle: false,
      eligibility,
      quotes,
      unavailable,
      settlement,
      available: null,
      quotedAt: (opts.now ?? Date.now)(),
    };
  };
}

/** A typed NAME as the registrar label, or null. */
export function nameLabel(value: string): NameLabel | null {
  return parseNameLabel(value.trim());
}
