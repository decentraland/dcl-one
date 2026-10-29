// from: shop@2d3bf62:app/src/pages/Success.tsx (useSettlement)

import { useEffect, useState } from "react";

import { useShopServices } from "../checkout/services";
import { waitForReceipt } from "../checkout/chain/rpc";
import { fetchOwnsItem } from "../checkout/lib/api";

export type Settlement = "pending" | "indexing" | "confirmed" | "failed" | "timed-out";

export type OwnershipCheck = { owner: string; contractAddress: string; itemId: string };

export function useSettlement(
  txs: { hash: string; chainId: number }[],
  opts: { skip?: boolean; attempts?: number; windowMs?: number; ownership?: OwnershipCheck | null } = {},
): Settlement {
  const { reader, wait, client } = useShopServices();
  const key = txs.map((t) => `${t.chainId}:${t.hash}`).join(",");
  const own = opts.ownership;
  const ownerKey = own ? `${own.owner}-${own.contractAddress}-${own.itemId}` : "";
  const [state, setState] = useState<Settlement>(opts.skip || txs.length === 0 ? "confirmed" : "pending");
  useEffect(() => {
    if (opts.skip || txs.length === 0) {
      setState("confirmed");
      return;
    }
    let cancelled = false;
    const attempts = opts.attempts ?? 8;
    const sleep = wait?.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
    const run = async () => {
      const outcomes = await Promise.all(
        txs.map(async (tx) => {
          for (let i = 0; i < attempts && !cancelled; i++) {
            const receipt = await waitForReceipt(reader, tx.chainId, tx.hash, { timeoutMs: opts.windowMs ?? wait?.timeoutMs ?? 15_000, pollMs: wait?.pollMs, sleep: wait?.sleep });
            if (receipt) return receipt.status;
          }
          return null;
        }),
      );
      if (cancelled) return;
      if (outcomes.some((o) => o === "reverted")) return setState("failed");
      if (outcomes.some((o) => o === null)) return setState("timed-out");
      if (!own) return setState("confirmed");
      setState("indexing");
      for (let i = 0; i < 40 && !cancelled; i++) {
        if (await fetchOwnsItem(own.owner, own.contractAddress, own.itemId, client)) {
          if (!cancelled) setState("confirmed");
          return;
        }
        await sleep(3000);
      }
      if (!cancelled) setState("timed-out");
    };
    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, opts.skip, ownerKey]);
  return state;
}
