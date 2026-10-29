// from: shop@94584a5c:app/src/lib/issue.ts
// "Issue copies": the creator mints fresh copies of their own published item to wallets, the builder's
// MintItemsModal flow. `ERC721CollectionV2.issueTokens(beneficiaries, itemIds)` mints one token per entry,
// so N copies to one address repeats it N times in the two aligned arrays. The creator is always a minter
// of their own collection. Gasless through the relay; a pending relay is never re-sent directly (it may
// still mint, and a second call would mint twice).

import { encodeFunctionData } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { COLLECTION_V2_ABI, collectionV2 } from "../../market/lib/contracts";
import { MetaTransactionError, sendMetaTransaction } from "../../market/lib/metaTx";
import { declined, isTxPending, sendCall, waitForReceipt, type WaitOptions } from "../../market/lib/tx";

export type IssueEntry = { address: string; amount: number };

export function isValidIssueAddress(addr: string): boolean {
  return /^0x[a-fA-F0-9]{40}$/.test(addr.trim());
}

/** All copies requested; blank or negative amounts count as zero. */
export function totalToIssue(entries: IssueEntry[]): number {
  return entries.reduce((sum, e) => sum + Math.max(0, Math.floor(e.amount || 0)), 0);
}

/** A submittable batch: at least one copy, every non-empty row valid, the total within the remaining supply. */
export function isIssueValid(entries: IssueEntry[], available: number): boolean {
  const rows = entries.filter((e) => e.address.trim().length > 0 || (e.amount || 0) > 0);
  if (rows.length === 0) return false;
  for (const e of rows) {
    if (!isValidIssueAddress(e.address)) return false;
    if (!Number.isInteger(e.amount) || e.amount < 1) return false;
  }
  const total = totalToIssue(rows);
  return total >= 1 && total <= available;
}

/** The two index-aligned arrays `issueTokens` takes; `itemId` is the on-chain item index. */
export function buildIssueArgs(entries: IssueEntry[], itemId: string): { beneficiaries: string[]; itemIds: string[] } {
  const beneficiaries: string[] = [];
  const itemIds: string[] = [];
  for (const entry of entries) {
    const address = entry.address.trim();
    const n = Math.max(0, Math.floor(entry.amount || 0));
    if (!address || n === 0) continue;
    for (let i = 0; i < n; i++) {
      beneficiaries.push(address);
      itemIds.push(itemId);
    }
  }
  return { beneficiaries, itemIds };
}

/** Upstream `issueTokens`: one batch, gasless first; resolves with the confirmed hash. */
export async function issueTokens(opts: {
  provider: Eip1193Provider;
  contractAddress: string;
  chainId: ChainId;
  entries: IssueEntry[];
  itemId: string;
  gasless?: boolean;
  wait?: WaitOptions;
}): Promise<string> {
  const { beneficiaries, itemIds } = buildIssueArgs(opts.entries, opts.itemId);
  if (beneficiaries.length === 0) throw new Error("No copies to issue");
  const args = [beneficiaries as `0x${string}`[], itemIds.map((id) => BigInt(id))] as const;
  if (opts.gasless !== false) {
    try {
      const hash = await sendMetaTransaction({
        provider: opts.provider,
        contract: collectionV2(opts.chainId, opts.contractAddress),
        kind: "collection",
        functionData: encodeFunctionData({ abi: COLLECTION_V2_ABI, functionName: "issueTokens", args }),
      });
      return await waitForReceipt(opts.chainId, hash, "The mint", opts.wait);
    } catch (e) {
      if (declined(e) || isTxPending(e) || !(e instanceof MetaTransactionError)) throw e;
    }
  }
  const hash = await sendCall(opts.provider, { chainId: opts.chainId, to: opts.contractAddress, abi: COLLECTION_V2_ABI, functionName: "issueTokens", args });
  return waitForReceipt(opts.chainId, hash, "The mint", opts.wait);
}
