// What a MANA buy dialog shows while its purchase, or the MANA approval before it, is still confirming: one copy
// for the LAND trade and the legacy order dialogs, with the transaction to watch and only Close (a second Buy
// could send a second approval).

import Button from "../../../atoms/Button";
import type { ChainId } from "@shop/model/chain";
import { explorerTxUrl } from "../../checkout/chain/config";
import { APPROVAL_STILL_PENDING, SALE_STILL_PENDING, type PurchaseFailure } from "../../market/lib/tx";

export type Confirming = Extract<PurchaseFailure, { kind: "pending" | "approval-pending" }>;

export default function ConfirmingNotice({ outcome, chainId, onClose }: { outcome: Confirming; chainId: ChainId; onClose(): void }) {
  const href = explorerTxUrl(chainId, outcome.txHash);
  return (
    <>
      <p role="status">{outcome.kind === "pending" ? SALE_STILL_PENDING : APPROVAL_STILL_PENDING}</p>
      {href ? (
        <p>
          <a href={href} target="_blank" rel="noreferrer">
            View transaction
          </a>
        </p>
      ) : null}
      <Button onClick={onClose}>Close</Button>
    </>
  );
}
