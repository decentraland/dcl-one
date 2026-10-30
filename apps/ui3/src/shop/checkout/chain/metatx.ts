// from: shop@94584a5c:app/src/lib/buy-gasless.ts (relay, GaslessUnavailableError, the signature normalisation),
//       decentraland-transactions@3.3.0:src/sendMetaTransaction.ts, src/utils.ts, src/types.ts (DOMAIN_TYPE,
//       META_TRANSACTION_TYPE, OFFCHAIN_META_TRANSACTION_TYPE)
// A relayed call: the buyer signs an EIP-712 MetaTransaction off-chain (no gas, no network switch:
// the chain id is the domain's `salt`, not its `chainId`), and the relay submits
// `executeMetaTransaction` and pays. The nonce is read from the target over our RPC, exactly as the
// relay re-reads it to recover the signer, so both sides hash the same digest.

import { decodeFunctionResult, encodeFunctionData, pad, toHex, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { NATIVE_META_TX_ABI, OFFCHAIN_META_TX_ABI } from "./abi";
import { relayerUrl as configuredRelayer } from "./config";
import type { MetaTxStyle } from "./contracts";
import type { ChainReader } from "./rpc";

export type MetaTxTarget = {
  address: string;
  chainId: ChainId;
  name: string;
  version: string;
  metaTx: MetaTxStyle;
};

/**
 * Why a relayed rail could not run. `relayer-rejected` vs `relayer-unreachable` is a money distinction:
 * a parsed answer with no hash was provably not broadcast, while no usable answer may have been.
 */
export type GaslessReason = "disabled" | "contract-account" | "signing-failed" | "relayer-rejected" | "relayer-unreachable" | "unknown";

export class GaslessUnavailableError extends Error {
  override name = "GaslessUnavailableError";
  constructor(
    message: string,
    readonly reason: GaslessReason = "unknown",
  ) {
    super(message);
  }
}

/** The buyer dismissed the signature prompt: a cancellation, never a reason to try another rail. */
export class SignatureDeclinedError extends Error {
  override name = "SignatureDeclinedError";
  readonly code = 4001;
}

export const DOMAIN_TYPE = [
  { name: "name", type: "string" },
  { name: "version", type: "string" },
  { name: "verifyingContract", type: "address" },
  { name: "salt", type: "bytes32" },
] as const;

export function metaTxType(style: MetaTxStyle) {
  return [
    { name: "nonce", type: "uint256" },
    { name: "from", type: "address" },
    { name: style, type: "bytes" },
  ] as const;
}

/** bytes32(chainId): Decentraland's contracts sign the chain into the domain `salt`. */
export function chainIdSalt(chainId: ChainId): Hex {
  return pad(toHex(chainId), { size: 32 });
}

export function metaTxTypedData(target: MetaTxTarget, from: string, nonce: bigint, functionData: Hex) {
  return {
    types: { EIP712Domain: DOMAIN_TYPE, MetaTransaction: metaTxType(target.metaTx) },
    domain: { name: target.name, version: target.version, verifyingContract: target.address, salt: chainIdSalt(target.chainId) },
    primaryType: "MetaTransaction" as const,
    message: { nonce: nonce.toString(), from, [target.metaTx]: functionData },
  };
}

export async function readMetaTxNonce(reader: ChainReader, target: Pick<MetaTxTarget, "address" | "chainId" | "metaTx">, from: string): Promise<bigint> {
  const abi = target.metaTx === "functionData" ? OFFCHAIN_META_TX_ABI : NATIVE_META_TX_ABI;
  const data = encodeFunctionData({ abi, functionName: "getNonce", args: [from as Hex] });
  const raw = await reader.call(target.chainId, target.address, data);
  return decodeFunctionResult({ abi, functionName: "getNonce", data: raw }) as bigint;
}

/**
 * Wallets disagree on the recovery id (0/1 vs 27/28); OpenZeppelin's ECDSA only takes 27/28. Contract
 * (EIP-1271) signatures of any other length pass through untouched.
 */
export function normalizeSignature(signature: Hex): Hex {
  if (!/^0x[0-9a-fA-F]{130}$/.test(signature)) return signature;
  const v = Number.parseInt(signature.slice(130), 16);
  return `${signature.slice(0, 130)}${(v < 27 ? v + 27 : v).toString(16).padStart(2, "0")}` as Hex;
}

function splitSignature(signature: Hex): { r: Hex; s: Hex; v: number } {
  const hex = normalizeSignature(signature);
  if (!/^0x[0-9a-fA-F]{130}$/.test(hex)) throw new Error("Not a 65-byte signature");
  return { r: `0x${hex.slice(2, 66)}` as Hex, s: `0x${hex.slice(66, 130)}` as Hex, v: Number.parseInt(hex.slice(130), 16) };
}

/** `executeMetaTransaction` calldata for the target's style. */
export function encodeExecuteMetaTransaction(style: MetaTxStyle, from: string, functionData: Hex, signature: Hex): Hex {
  if (style === "functionData") {
    return encodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, functionName: "executeMetaTransaction", args: [from as Hex, functionData, signature] });
  }
  const { r, s, v } = splitSignature(signature);
  return encodeFunctionData({ abi: NATIVE_META_TX_ABI, functionName: "executeMetaTransaction", args: [from as Hex, functionData, r, s, v] });
}

function declined(e: unknown): boolean {
  const err = e as { code?: unknown; message?: string } | null;
  return err?.code === 4001 || err?.code === "ACTION_REJECTED" || /denied|reject|cancel/i.test(err?.message ?? "");
}

export type RelayOptions = {
  provider: Eip1193Provider;
  reader: ChainReader;
  target: MetaTxTarget;
  from: string;
  functionData: Hex;
  relayerUrl?: string;
  fetchImpl?: typeof fetch;
  /** A nonce already observed on chain: never sign against a staler one (load-balanced RPCs lag). */
  minNonce?: bigint;
  /** Fired once the buyer signed (the prompt is gone), before the relay is called. */
  onSigned?: () => void;
};

export type Relayed = { txHash: Hex; nonce: bigint };

export async function relayMetaTransaction(opts: RelayOptions): Promise<Relayed> {
  const { provider, reader, target, from, functionData } = opts;
  const code = await reader.code(target.chainId, from).catch(() => "0x" as Hex);
  if (code && code !== "0x") throw new GaslessUnavailableError("Contract accounts are not supported", "contract-account");

  const read = await readMetaTxNonce(reader, target, from);
  const nonce = opts.minNonce !== undefined && opts.minNonce > read ? opts.minNonce : read;
  const typed = metaTxTypedData(target, from, nonce, functionData);

  let signature: Hex;
  try {
    signature = (await provider.request({ method: "eth_signTypedData_v4", params: [from, JSON.stringify(typed)] })) as Hex;
  } catch (e) {
    if (declined(e)) throw new SignatureDeclinedError((e as Error)?.message ?? "Signature declined");
    throw new GaslessUnavailableError((e as Error)?.message ?? "The wallet could not sign", "signing-failed");
  }
  signature = normalizeSignature(signature);
  opts.onSigned?.();

  const data = encodeExecuteMetaTransaction(target.metaTx, from, functionData, signature);
  const base = (opts.relayerUrl ?? configuredRelayer()).replace(/\/$/, "");
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  type RelayerBody = { ok?: boolean; txHash?: string; message?: string; code?: unknown };
  let body: RelayerBody | null = null;
  let status = 0;
  try {
    const res = await fetchImpl(`${base}/transactions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ transactionData: { from, params: [target.address, data] } }),
    });
    status = res.status;
    body = (await res.json().catch(() => null)) as RelayerBody | null;
  } catch (e) {
    throw new GaslessUnavailableError((e as Error)?.message ?? "relayer unreachable", "relayer-unreachable");
  }
  if (!body) throw new GaslessUnavailableError(`relayer answered ${status} with no body`, "relayer-unreachable");
  if (body.ok === false || !body.txHash) {
    throw new GaslessUnavailableError(body.message ?? `relayer ${status}`, "relayer-rejected");
  }
  return { txHash: body.txHash as Hex, nonce };
}

export function isGaslessUnavailable(e: unknown): e is GaslessUnavailableError {
  return e instanceof GaslessUnavailableError || (e as { name?: string } | null)?.name === "GaslessUnavailableError";
}

export function isSignatureDeclined(e: unknown): boolean {
  return e instanceof SignatureDeclinedError || (e as { name?: string } | null)?.name === "SignatureDeclinedError";
}
