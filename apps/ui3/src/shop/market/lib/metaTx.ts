// from: shop@94584a5c:app/src/lib/buy.ts (cancelViaMetaTransaction, transferViaMetaTransaction),
//       app/src/lib/issue.ts (issueViaMetaTransaction); decentraland-transactions@3.3.0 src/sendMetaTransaction.ts
// Gasless writes on Polygon: the owner signs an EIP-712 MetaTransaction and the shared relay
// (`/transactions-api/v1/transactions`, v2's economy module) submits it and pays the gas. The collection
// contracts take `executeMetaTransaction(address,bytes,bytes32,bytes32,uint8)`, the off-chain
// marketplace the `(address,bytes,bytes)` variant; both recover the signer from the same typed data,
// differing only in the message field's name. A relay refusal is an answer with its code; a declined
// signature is the owner's answer and is never retried as a gas-paying transaction.

import { encodeFunctionData, parseAbi } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { catalystBase } from "../../../data/catalyst/client";
import type { ContractData } from "./contracts";
import { rpc } from "./rpc";
import { chainSalt, signTypedData, walletAccount } from "./tx";

const NONCE_ABI = parseAbi(["function getNonce(address user) view returns (uint256 nonce)"]);
const EXECUTE_ABI = parseAbi([
  "function executeMetaTransaction(address userAddress, bytes functionSignature, bytes32 sigR, bytes32 sigS, uint8 sigV) payable returns (bytes)",
]);
const OFFCHAIN_EXECUTE_ABI = parseAbi(["function executeMetaTransaction(address _userAddress, bytes _functionData, bytes _signature) payable returns (bytes)"]);

export class MetaTransactionError extends Error {
  override name = "MetaTransactionError";
  constructor(
    message: string,
    readonly code: string,
  ) {
    super(message);
  }
}

type RelayWindow = Window & { __SHOP_RELAY__?: string };

/** The relay's base; POSTs go to `${relayBase()}/transactions` (decentraland-transactions `serverURL`). */
export function relayBase(): string {
  const fromWindow = typeof window !== "undefined" ? (window as RelayWindow).__SHOP_RELAY__ : undefined;
  return fromWindow ?? `${catalystBase()}/transactions-api/v1`;
}

/** Ledger answers v as 0/1; the contracts recover only 27/28. */
export function normalizeV(v: number): number {
  return v < 27 ? v + 27 : v;
}

export function splitSignature(signature: `0x${string}`): { r: `0x${string}`; s: `0x${string}`; v: number } {
  const hex = signature.slice(2);
  if (hex.length !== 130) throw new Error("Expected a 65-byte signature");
  return { r: `0x${hex.slice(0, 64)}`, s: `0x${hex.slice(64, 128)}`, v: normalizeV(Number.parseInt(hex.slice(128, 130), 16)) };
}

export type MetaTxKind = "collection" | "offchain";

/** The typed data a meta-transaction signs (decentraland-transactions `getDataToSign`). */
export function metaTransactionTypedData(contract: ContractData, kind: MetaTxKind, from: string, nonce: bigint, functionData: `0x${string}`) {
  const field = kind === "offchain" ? "functionData" : "functionSignature";
  return {
    domain: { name: contract.name, version: contract.version, verifyingContract: contract.address, salt: chainSalt(contract.chainId) },
    types: {
      MetaTransaction: [
        { name: "nonce", type: "uint256" },
        { name: "from", type: "address" },
        { name: field, type: "bytes" },
      ],
    },
    primaryType: "MetaTransaction",
    message: { nonce, from, [field]: functionData },
  };
}

/** The calldata the relay submits for a signed meta-transaction. */
export function executeMetaTransactionData(kind: MetaTxKind, from: string, signature: `0x${string}`, functionData: `0x${string}`): `0x${string}` {
  if (kind === "offchain") {
    const { r, s, v } = splitSignature(signature);
    const normalized = `${r}${s.slice(2)}${v.toString(16).padStart(2, "0")}` as `0x${string}`;
    return encodeFunctionData({ abi: OFFCHAIN_EXECUTE_ABI, functionName: "executeMetaTransaction", args: [from as `0x${string}`, functionData, normalized] });
  }
  const { r, s, v } = splitSignature(signature);
  return encodeFunctionData({ abi: EXECUTE_ABI, functionName: "executeMetaTransaction", args: [from as `0x${string}`, functionData, r, s, v] });
}

async function readNonce(contract: ContractData, from: string): Promise<bigint> {
  const data = encodeFunctionData({ abi: NONCE_ABI, functionName: "getNonce", args: [from as `0x${string}`] });
  const raw = await rpc<string>(contract.chainId, "eth_call", [{ to: contract.address, data }, "latest"]);
  return BigInt(raw);
}

/**
 * Signs `functionData` as a meta-transaction on `contract` and hands it to the relay. Returns the relay's
 * transaction hash (the relay may re-submit with a higher fee: confirm by effect where one is readable).
 */
export async function sendMetaTransaction(opts: {
  provider: Eip1193Provider;
  contract: ContractData;
  kind: MetaTxKind;
  functionData: `0x${string}`;
  fetchImpl?: typeof fetch;
}): Promise<string> {
  const from = await walletAccount(opts.provider);
  const nonce = await readNonce(opts.contract, from);
  const signature = await signTypedData(opts.provider, from, metaTransactionTypedData(opts.contract, opts.kind, from, nonce, opts.functionData));
  const txData = executeMetaTransactionData(opts.kind, from, signature, opts.functionData);
  const res = await (opts.fetchImpl ?? fetch)(`${relayBase()}/transactions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ transactionData: { from, params: [opts.contract.address, txData] } }),
  });
  const body = (await res.json().catch(() => null)) as { ok?: boolean; txHash?: string; message?: string; code?: string } | null;
  if (!body || body.ok === false || !body.txHash) {
    throw new MetaTransactionError(body?.message ?? `The relay answered ${res.status}`, body?.code ?? String(res.status));
  }
  return body.txHash;
}
