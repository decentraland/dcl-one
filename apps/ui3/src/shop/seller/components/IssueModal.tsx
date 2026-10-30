// from: shop@94584a5c:app/src/components/IssueModal/IssueModal.tsx, app/src/lib/issue.ts
// Issue copies: the creator mints fresh copies of their published item to one or more addresses, up to the
// remaining supply, in one gasless batch. The rows are checked before any prompt.

import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import Button from "../../../atoms/Button";
import { CHAIN, type ChainId } from "../../model/chain";
import { useShopProvider, useWalletBlock, useWrite, WriteStatus } from "../../market/components/kit";
import type { PublishableItem } from "../lib/builder";
import { isIssueValid, isValidIssueAddress, issueTokens, totalToIssue, type IssueEntry } from "../lib/issue";

export function IssueForm({ item, chainId = CHAIN.POLYGON, onClose }: { item: PublishableItem; chainId?: ChainId; onClose: () => void }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const block = useWalletBlock("Issuing copies");
  const [rows, setRows] = useState<IssueEntry[]>([{ address: "", amount: 1 }]);
  const write = useWrite();
  const total = totalToIssue(rows);
  const valid = isIssueValid(rows, item.remainingSupply);

  if (block || !provider) return block;
  if (write.state.kind === "done")
    return (
      <div className="mk-section" data-testid="shop-issue-done">
        <p role="status">
          Issued {total} {total === 1 ? "copy" : "copies"} of “{item.name}”.
        </p>
        <Button size="sm" variant="ghost" onClick={onClose}>
          Close
        </Button>
      </div>
    );
  return (
    <form
      className="mk-panel mk-form"
      aria-label={`Issue copies of ${item.name}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        void write.run("Confirm in your wallet (no gas needed)…", "Issued.", async () => {
          const hash = await issueTokens({ provider, contractAddress: item.contractAddress, chainId, entries: rows, itemId: item.blockchainItemId });
          await qc.invalidateQueries({ queryKey: ["store-catalogue"] });
          return hash;
        });
      }}
    >
      <h2 className="mk-panel__title">Issue copies</h2>
      <p className="mk-hint">Send fresh copies of “{item.name}” to any address. {item.remainingSupply.toLocaleString("en-US")} left to issue.</p>
      {rows.map((row, i) => (
        <div className="mk-actions" key={i} role="group" aria-label={`Recipient ${i + 1}`}>
          <label className="mk-field">
            <span className="mk-field__label">Address</span>
            <input value={row.address} placeholder="0x…" onChange={(e) => setRows(rows.map((r, j) => (j === i ? { ...r, address: e.target.value } : r)))} disabled={write.busy} />
            {row.address.trim() && !isValidIssueAddress(row.address) ? <p className="mk-field__error">Enter a valid address (0x…).</p> : null}
          </label>
          <label className="mk-field">
            <span className="mk-field__label">Amount</span>
            <input type="number" min={1} value={row.amount} onChange={(e) => setRows(rows.map((r, j) => (j === i ? { ...r, amount: Number(e.target.value) } : r)))} disabled={write.busy} />
          </label>
          {rows.length > 1 ? (
            <Button size="sm" variant="ghost" onClick={() => setRows(rows.filter((_, j) => j !== i))} disabled={write.busy}>
              Remove recipient
            </Button>
          ) : null}
        </div>
      ))}
      <div className="mk-actions">
        <Button size="sm" variant="secondary" onClick={() => setRows([...rows, { address: "", amount: 1 }])} disabled={write.busy}>
          Add recipient
        </Button>
        <span className="mk-hint">
          {total} {total === 1 ? "copy" : "copies"} to issue
        </span>
      </div>
      {total > item.remainingSupply ? <p className="mk-field__error">You can issue at most {item.remainingSupply} more copies of this item.</p> : null}
      <div className="mk-actions">
        <Button type="submit" disabled={!valid || write.busy}>
          {write.busy ? "Issuing…" : `Issue ${total} ${total === 1 ? "copy" : "copies"}`}
        </Button>
        <Button variant="ghost" onClick={onClose} disabled={write.busy}>
          Cancel
        </Button>
      </div>
      <WriteStatus state={write.state} />
    </form>
  );
}
