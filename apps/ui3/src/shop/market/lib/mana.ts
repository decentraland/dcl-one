// from: shop@94584a5c:app/src/lib/authorizations.ts (getManaSpendingAuthorization, needsApprovalStep),
//       app/src/hooks/useManaBalance.ts; marketplace@f3956826:webapp/src/modules/bid/utils.ts (isInsufficientMANA)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// MANA the wallet holds and has allowed a contract to spend, read from the target chain, and the one
// allowance a MANA bid or a MANA purchase needs before it can settle. On Polygon the approval is relayed
// (MANA (PoS) supports native meta-transactions); on Ethereum the wallet sends it and pays gas.

import { encodeFunctionData } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { isPolygon, type ChainId } from "../../model/chain";
import type { ManaWei } from "../../model/money";
import { ERC20_ABI, manaToken } from "./contracts";
import { sendMetaTransaction } from "./metaTx";
import { readContract } from "./rpc";
import { sendCall, waitForReceipt, walletAccount, type WaitOptions } from "./tx";

/** A typed MANA amount ("12.5") as exact wei; null for anything that is not a positive amount with at most 18 decimals. */
export function parseManaInput(text: string): ManaWei | null {
  const t = text.trim();
  if (!/^\d+(\.\d{1,18})?$/.test(t)) return null;
  const [whole, fraction = ""] = t.split(".");
  const wei = BigInt(whole!) * 10n ** 18n + BigInt(fraction.padEnd(18, "0") || "0");
  return wei > 0n ? (wei as ManaWei) : null;
}

export async function readManaBalance(chainId: ChainId, owner: string): Promise<ManaWei> {
  const mana = manaToken(chainId);
  return (await readContract({ chainId, address: mana.address, abi: ERC20_ABI, functionName: "balanceOf", args: [owner] })) as ManaWei;
}

export async function readManaAllowance(chainId: ChainId, owner: string, spender: string): Promise<bigint> {
  const mana = manaToken(chainId);
  return (await readContract({ chainId, address: mana.address, abi: ERC20_ABI, functionName: "allowance", args: [owner, spender] })) as bigint;
}

/** Whether `spender` may already move `amount` of the owner's MANA (no approval step needed). */
export async function hasManaAllowance(chainId: ChainId, owner: string, spender: string, amount: bigint): Promise<boolean> {
  return (await readManaAllowance(chainId, owner, spender)) >= amount;
}

/**
 * Approves `spender` for `amount` MANA when the current allowance is short. Resolves null when nothing was
 * needed, else the confirmed hash. The exact amount, never an unlimited allowance.
 */
export async function ensureManaAllowance(opts: {
  provider: Eip1193Provider;
  chainId: ChainId;
  spender: string;
  amount: bigint;
  wait?: WaitOptions;
}): Promise<string | null> {
  const owner = await walletAccount(opts.provider);
  if (await hasManaAllowance(opts.chainId, owner, opts.spender, opts.amount)) return null;
  const mana = manaToken(opts.chainId);
  const hash = isPolygon(opts.chainId)
    ? await sendMetaTransaction({
        provider: opts.provider,
        contract: mana,
        kind: "collection",
        functionData: encodeFunctionData({ abi: ERC20_ABI, functionName: "approve", args: [opts.spender as `0x${string}`, opts.amount] }),
      })
    : await sendCall(opts.provider, { chainId: opts.chainId, to: mana.address, abi: ERC20_ABI, functionName: "approve", args: [opts.spender, opts.amount] });
  return waitForReceipt(opts.chainId, hash, "The MANA approval", opts.wait);
}
