// Mirrors src/shop/policy.rs `restrictions` and `present`, for PRESENTATION only: the server decides
// and its PurchaseOffer is what the UI renders. The mirror exists so presentPayment can assert that
// the server's answer is consistent with the confirmed rule; it reads the kind and sale the server
// derived, never a client-side "primary" flag.

import type { AssetKind, SaleKind } from "../model/asset";
import { isPolygon, nativeGasRequired, type ChainId } from "../model/chain";
import type { PurchaseOffer } from "../model/purchase";
import type { Eligibility, Reason, Session } from "../model/settlement";

/** The restrictions that make a line MANA only; empty means credits-eligible. */
export function restrictions(kind: AssetKind, sale: SaleKind, chainId: ChainId, bundle: boolean): Reason[] {
  const reasons: Reason[] = [];
  if (kind === "parcel" || kind === "estate" || kind === "rental") reasons.push("landOrRental");
  if (sale === "secondary") reasons.push("secondarySale");
  if (kind === "other") reasons.push("notDecentralandItem");
  if (bundle) reasons.push("bundle");
  const primaryPair =
    (sale === "primaryMint" && (kind === "wearable" || kind === "emote")) ||
    (sale === "primaryNameRegistration" && kind === "name");
  if (reasons.length === 0 && !primaryPair) reasons.push("unsupportedCombination");
  if (reasons.length === 0 && sale === "primaryMint" && !isPolygon(chainId)) reasons.push("creditsChainUnsupported");
  return reasons;
}

export function present(restricted: Reason[], session: Session, manaChain: ChainId): Eligibility {
  if (restricted.length === 0) {
    switch (session.kind) {
      case "web3":
        return { allowed: ["credits", "mana"], default: "credits", reasons: ["creditsFirst", "manaSecond"], guidance: null };
      case "web2":
        return { allowed: ["credits"], default: "credits", reasons: ["creditsFirst"], guidance: null };
      case "signedOut":
        return { allowed: ["credits"], default: "credits", reasons: ["creditsFirst", "signInForCredits"], guidance: null };
    }
  }
  const guidance =
    session.kind === "web3"
      ? null
      : {
          chainId: manaChain,
          nativeGasRequired: nativeGasRequired(manaChain),
          account: session.kind === "web2" ? session.account : null,
        };
  return { allowed: ["mana"], default: "mana", reasons: [...restricted], guidance };
}

/**
 * The sale a line's identity allows: a mint sells a collection item, a registration or a resale an
 * existing (or registrar) token. A server answer that pairs a primary sale with the wrong identity is
 * judged as the secondary sale its identity says it is.
 */
export function identitySale(offer: Pick<PurchaseOffer, "asset" | "kind" | "sale">): SaleKind {
  const form = offer.asset.token.kind;
  if (offer.sale === "primaryMint") return form === "item" ? "primaryMint" : "secondary";
  if (offer.sale === "primaryNameRegistration") {
    return form === "token" && offer.kind === "name" ? "primaryNameRegistration" : "secondary";
  }
  return "secondary";
}

/** The chain a MANA payment settles on: the MANA settlement key's, else the asset's (`policy::eligibility`). */
export function manaChainOf(offer: Pick<PurchaseOffer, "asset" | "settlement">): ChainId {
  return offer.settlement.find((s) => s.method === "mana")?.key.chainId ?? offer.asset.chainId;
}

/** THE rule, as the UI mirrors it for one server-derived line. */
export function ruleFor(
  offer: Pick<PurchaseOffer, "asset" | "kind" | "sale" | "bundle" | "settlement">,
  session: Session,
): Eligibility {
  const sale = identitySale(offer);
  return present(restrictions(offer.kind, sale, offer.asset.chainId, offer.bundle), session, manaChainOf(offer));
}
