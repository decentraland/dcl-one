// from: shop@94584a5c:app/src/lib/tx-confirm.ts, app/src/lib/buy.ts (the direct, wallet-broadcast legs)
// Wallet writes on ui3's EIP-1193 seam instead of an ethers signer. The wallet broadcasts, so it must
// already be on the transaction's chain: this only CHECKS (`requireChain`), moving the wallet is the
// seller's own click (the network notice's switch button), never a side effect of the action. A receipt
// is read from the target chain's RPC; a mined revert is final, a missing receipt is "pending", never a
// failure to retry around (re-sending could run a transfer or a mint twice).

import { encodeFunctionData, type Abi } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { isUserRejection, requireChain } from "../../wallet/chain";
import { rpc } from "./rpc";

export class TxRevertedError extends Error {
  override name = "TxRevertedError";
  constructor(readonly txHash: string) {
    super("The transaction was reverted on chain.");
  }
}

/** No receipt yet: the transaction may still mine. Show it as pending, never retry it. */
export class TxPendingError extends Error {
  override name = "TxPendingError";
  constructor(
    readonly txHash: string,
    what: string,
  ) {
    super(`${what} is still pending. It may still complete; check again in a moment.`);
  }
}

export function isTxPending(e: unknown): e is TxPendingError {
  return (e as { name?: string } | null)?.name === "TxPendingError";
}

export function isTxReverted(e: unknown): e is TxRevertedError {
  return (e as { name?: string } | null)?.name === "TxRevertedError";
}

/** The wallet declined (4001), or said so in words (EIP-1193 wallets vary). */
export function declined(e: unknown): boolean {
  if (isUserRejection(e)) return true;
  return /user (rejected|denied)/i.test((e as { message?: string } | null)?.message ?? "");
}

export async function walletAccount(provider: Eip1193Provider): Promise<string> {
  const accounts = (await provider.request({ method: "eth_accounts" })) as string[] | undefined;
  const account = accounts?.[0];
  if (!account) throw new Error("Connect your wallet to continue.");
  return account.toLowerCase();
}

export type WriteCall = {
  chainId: ChainId;
  to: string;
  abi: Abi;
  functionName: string;
  args: readonly unknown[];
  value?: bigint;
};

/** Sends one contract call from the wallet. Throws WrongNetworkError without prompting on another chain. */
export async function sendCall(provider: Eip1193Provider, call: WriteCall): Promise<string> {
  await requireChain(provider, call.chainId);
  const from = await walletAccount(provider);
  const data = encodeFunctionData({ abi: call.abi, functionName: call.functionName, args: call.args } as never);
  const tx: Record<string, string> = { from, to: call.to, data };
  if (call.value) tx.value = `0x${call.value.toString(16)}`;
  const hash = (await provider.request({ method: "eth_sendTransaction", params: [tx] })) as string;
  if (typeof hash !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(hash)) throw new Error("The wallet did not return a transaction hash.");
  return hash;
}

type Receipt = { status?: string; transactionHash?: string; blockNumber?: string } | null;

export type WaitOptions = { timeoutMs?: number; pollMs?: number; sleep?: (ms: number) => Promise<void>; now?: () => number };

const realSleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Polls the target chain for the receipt. Resolves on success; TxRevertedError on a revert; TxPendingError on timeout. */
export async function waitForReceipt(chainId: ChainId, txHash: string, what = "The transaction", opts: WaitOptions = {}): Promise<string> {
  const timeoutMs = opts.timeoutMs ?? 180_000;
  const pollMs = opts.pollMs ?? 3_000;
  const sleep = opts.sleep ?? realSleep;
  const now = opts.now ?? Date.now;
  const started = now();
  for (;;) {
    let receipt: Receipt = null;
    try {
      receipt = await rpc<Receipt>(chainId, "eth_getTransactionReceipt", [txHash]);
    } catch {
      // A failed read is not a revert: keep waiting until the deadline.
    }
    if (receipt?.status === "0x1") return txHash;
    if (receipt?.status === "0x0") throw new TxRevertedError(txHash);
    if (now() - started >= timeoutMs) throw new TxPendingError(txHash, what);
    await sleep(pollMs);
  }
}

export type TypedData = {
  domain: Record<string, unknown>;
  types: Record<string, readonly { name: string; type: string }[]>;
  primaryType: string;
  message: Record<string, unknown>;
};

function jsonSafe(value: unknown): unknown {
  if (typeof value === "bigint") return value.toString();
  if (Array.isArray(value)) return value.map(jsonSafe);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, jsonSafe(v)]));
  return value;
}

/** `eth_signTypedData_v4`, with the EIP712Domain type derived from the domain's fields. */
export async function signTypedData(provider: Eip1193Provider, account: string, data: TypedData): Promise<`0x${string}`> {
  const domainFields: { name: string; type: string }[] = [];
  if ("name" in data.domain) domainFields.push({ name: "name", type: "string" });
  if ("version" in data.domain) domainFields.push({ name: "version", type: "string" });
  if ("chainId" in data.domain) domainFields.push({ name: "chainId", type: "uint256" });
  if ("verifyingContract" in data.domain) domainFields.push({ name: "verifyingContract", type: "address" });
  if ("salt" in data.domain) domainFields.push({ name: "salt", type: "bytes32" });
  const payload = JSON.stringify(
    jsonSafe({ types: { EIP712Domain: domainFields, ...data.types }, domain: data.domain, primaryType: data.primaryType, message: data.message }),
  );
  const signature = (await provider.request({ method: "eth_signTypedData_v4", params: [account, payload] })) as string;
  if (typeof signature !== "string" || !signature.startsWith("0x")) throw new Error("The wallet returned an invalid signature.");
  return signature as `0x${string}`;
}

/** The chain id as a 32-byte salt: the Decentraland contracts' EIP-712 domain carries the chain there. */
export function chainSalt(chainId: ChainId): `0x${string}` {
  return `0x${chainId.toString(16).padStart(64, "0")}`;
}
