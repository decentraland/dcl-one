// from: shop@94584a5c:app/src/components/CartCheckoutModal/CartCheckoutModal.tsx (processing stages,
//       the signature counter, the held-credits failure), app/src/pages/Cart.tsx (partial purchases)
// The explicit confirmation groups: one per payment method, chain and settlement contract, each with
// what it pays, and during and after checkout its own progress (reserving, waiting for the network,
// confirmed, failed with the reason). Different chains cannot promise an atomic basket, so a group is
// never reported as done because another one was.

import type { CartGroup } from "../../cart/partition";
import { chainLabel } from "../../model/chain";
import { creditsFloor, formatUsd } from "../../model/money";
import type { CartLine, GroupProgress } from "../../model/purchase";
import { CreditsAmount, ManaAmount } from "../../credits/components/Amount";
import { explorerTxUrl } from "../chain/config";
import { contractAt } from "../chain/contracts";
import type { GroupResult } from "../purchase/executor";
import "./checkoutgroups.css";

type Row = Pick<CartGroup, "keyId" | "key" | "lineIds"> & { total: CartGroup["total"] | null };

function venue(group: Row): string {
  const known = contractAt(group.key.chainId, group.key.contract)?.label;
  const call = group.key.call.kind;
  if (call === "storeBuy") return "Collection store";
  if (known?.startsWith("OffChainMarketplace") || call === "accept" || call === "acceptWithCoupon") return call === "acceptWithCoupon" ? "Marketplace, on sale" : "Marketplace";
  return known ?? "Contract";
}

function statusText(group: Row, progress: GroupProgress | undefined, result: GroupResult | undefined, walletNeeded: boolean): { text: string; tone: "idle" | "busy" | "done" | "failed" | "wait" } {
  if (walletNeeded && (!progress || progress.status === "idle")) return { text: "Needs a web3 wallet with MANA", tone: "wait" };
  if (!progress) return { text: "Ready to confirm", tone: "idle" };
  switch (progress.status) {
    case "idle":
      return { text: "Ready to confirm", tone: "idle" };
    case "authorizing":
      return { text: group.key.method === "credits" ? "Reserving your Credits…" : "Preparing the MANA payment…", tone: "busy" };
    case "submitted":
      return { text: progress.txHash ? "Sent. Waiting for the network…" : "Sent. Confirm in your wallet if it asks…", tone: "busy" };
    case "pending":
      return { text: "Waiting for the network to confirm it. This can take a few minutes.", tone: "busy" };
    case "confirmed":
      return { text: "Done", tone: "done" };
    case "expired":
      return { text: "The reservation expired before it was used. Nothing was charged.", tone: "failed" };
    case "failed":
      return { text: progress.failure?.message ?? (result?.status === "failed" ? result.failure.message : "Failed"), tone: "failed" };
  }
}

export type CheckoutGroupsProps = {
  groups: CartGroup[];
  lines: CartLine[];
  progress: GroupProgress[];
  results: Record<string, GroupResult>;
  walletNeededKeys: string[];
};

export default function CheckoutGroups({ groups, lines, progress, results, walletNeededKeys }: CheckoutGroupsProps) {
  const names = new Map(lines.map((l) => [l.id, l.display.name]));
  const shown: Row[] = groups.map((g) => ({ keyId: g.keyId, key: g.key, lineIds: g.lineIds, total: g.total }));
  for (const p of progress) if (!shown.some((g) => g.keyId === p.keyId)) shown.push({ keyId: p.keyId, key: p.key, lineIds: p.lineIds, total: null });
  if (shown.length === 0) return null;

  return (
    <ol className="shop-groups" aria-label="Payments in this checkout">
      {shown.map((group, i) => {
        const p = progress.find((x) => x.keyId === group.keyId);
        const status = statusText(group, p, results[group.keyId], walletNeededKeys.includes(group.keyId));
        const tx = p?.txHash ? explorerTxUrl(group.key.chainId, p.txHash) : null;
        const total = group.total;
        return (
          <li key={group.keyId} className="shop-group" data-method={group.key.method} data-tone={status.tone} data-testid="checkout-group">
            <div className="shop-group__head">
              <span className="shop-group__title">
                {shown.length > 1 ? `${i + 1}. ` : ""}
                {group.key.method === "credits" ? "Pay with Credits" : `Pay with MANA on ${chainLabel(group.key.chainId)}`}
              </span>
              {total ? (
                total.method === "credits" ? (
                  <CreditsAmount credits={creditsFloor(total.charged)} />
                ) : (
                  <ManaAmount wei={total.amount} chainId={group.key.chainId} />
                )
              ) : null}
            </div>
            <p className="shop-group__via">
              {venue(group)} · {group.lineIds.map((id) => names.get(id) ?? "Item").join(", ")}
              {total?.method === "credits" && total.quoted > 0 && total.quoted !== total.charged ? ` · priced ${formatUsd(total.quoted)}, charged in whole Credits` : ""}
            </p>
            <p className="shop-group__status" role={status.tone === "failed" ? "alert" : "status"}>
              {status.text}
              {tx ? (
                <>
                  {" "}
                  <a href={tx} target="_blank" rel="noopener noreferrer">
                    View transaction
                  </a>
                </>
              ) : null}
            </p>
          </li>
        );
      })}
    </ol>
  );
}
