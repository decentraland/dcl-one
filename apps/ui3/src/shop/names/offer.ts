// The purchase offer the NAME registration screens render. The server is the authority: when the host
// wires the Shop's quote route (`serverNameOffer`), its PurchaseOffer is used as it comes. Until that
// route is mounted, `interimNameOffer` assembles the same shape from what the server and the chain
// already answer: the registrar's `available(name)` (a taken NAME is a resale, never a registration),
// the rule mirror of payment/rule.ts for kind `name` + sale `primaryNameRegistration`, the controller's
// fixed 100 MANA and the credits quote of `/shop-api/v1/credit-quotes`. It can only ever present the
// registration rule; every entry (the credit authorization with the NAME, the credit route, the
// registrar) re-checks it, and a credits quote the server cannot give is shown as unavailable, never
// replaced by MANA.

import { isShopApiError } from "../api/client";
import { missingOfferRoute, type OfferSource } from "../detail/offers";
import { shopClient, type ShopClient } from "../api/shop";
import { evmAddress } from "../model/address";
import { nameAsset, parseNameLabel, type NameLabel } from "../model/asset";
import { CHAIN, type ChainId } from "../model/chain";
import { centsFromCredits, manaWei, type UsdCents } from "../model/money";
import type { PurchaseOffer } from "../model/purchase";
import type { MissingFact, PaymentMethod, Quote, Session, SettlementGroupKey } from "../model/settlement";
import { decodePurchaseOffer } from "../model/wire";
import { present, restrictions } from "../payment/rule";
import { landChainId, landNetwork } from "../land/contracts";
import type { ChainReader } from "../land/evm";
import { checkNameAvailability, creditsChainFor, NAME_PRICE_IN_WEI, NAME_PRICE_WEI } from "./names";

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

/** The credits price of 100 MANA in cents (whole credits), or the reason it cannot be given. */
export async function quoteNameCredits(client: ShopClient, signal?: AbortSignal): Promise<{ quoted: UsdCents } | { missing: MissingFact }> {
  try {
    const body = await client.credits.quotes({ amounts: [NAME_PRICE_IN_WEI] }, { signal });
    const raw = body?.amounts?.[0];
    const credits = raw != null && /^[0-9]+$/.test(raw) ? Number(raw) : null;
    const cents = credits !== null && credits > 0 ? centsFromCredits(credits) : null;
    return cents !== null ? { quoted: cents } : { missing: "manaUsdRate" };
  } catch (e) {
    if ((e as { name?: string } | null)?.name === "AbortError") throw e;
    if (isShopApiError(e) || e instanceof Error) return { missing: "manaUsdRate" };
    throw e;
  }
}

export function interimNameOffer(opts: { client?: ShopClient; reader?: ChainReader; chainId?: ChainId; now?: () => number } = {}): NameOfferSource {
  return async (label, session, signal) => {
    const chainId = opts.chainId ?? landChainId();
    const network = landNetwork(chainId);
    const creditsChain = creditsChainFor(chainId);
    const client = opts.client ?? shopClient();
    const [availability, credits] = await Promise.all([
      checkNameAvailability(label, { signal, reader: opts.reader, chainId }),
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
