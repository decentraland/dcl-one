// from: shop@94584a5c:app/src/lib/buy-gasless.ts (relay, GaslessUnavailableError, the signature normalisation),
//       decentraland-transactions@3.3.0:src/sendMetaTransaction.ts, src/utils.ts, src/types.ts (DOMAIN_TYPE,
//       META_TRANSACTION_TYPE, OFFCHAIN_META_TRANSACTION_TYPE)
// A relayed call: the buyer signs an EIP-712 MetaTransaction off-chain (no gas, no network switch:
// the chain id is the domain's `salt`, not its `chainId`), and the relay submits
// `executeMetaTransaction` and pays. The nonce is read from the target over our RPC, exactly as the
// relay re-reads it to recover the signer, so both sides hash the same digest.

import { decodeFunctionData, decodeFunctionResult, encodeFunctionData, pad, toHex, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "@shop/model/chain";
import { NATIVE_META_TX_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";
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
 * `relayer-rejected` is a refusal the relay marks as not sent (`refusedBeforeBroadcast`), so nothing
 * was sent; `relayer-unreachable` is any other answer (none at all, a timeout, a failure without that
 * evidence, an answer without a hash), so the transaction may still land.
 */
export type GaslessReason = "disabled" | "contract-account" | "signing-failed" | "relayer-rejected" | "relayer-unreachable" | "unknown";

/**
 * What a relay's refusal says of itself, when it does: its code (its daily limits name themselves:
 * `quota_reached` for the account's own fee-free quota, `relay_budget_spent` for the relay's), and for a spent quota
 * when its next slot frees (`retryAt`, unix seconds) and how many it holds (`quota`).
 */
export type RelayRefusal = { error?: string; retryAt?: number; quota?: number };

/**
 * A relay answer's refusal fields, each kept only in its own shape. The code is read from `code` (the upstream
 * transactions-server's field, which dcl-one's relay also answers), else from `error` (dcl-one's own, and its only
 * one before it answered `code` too).
 */
export function relayRefusal(body: unknown): RelayRefusal {
  const answer = (body && typeof body === "object" ? body : {}) as { code?: unknown; error?: unknown; retryAt?: unknown; quota?: unknown };
  const positive = (n: unknown): n is number => typeof n === "number" && Number.isSafeInteger(n) && n > 0;
  // A quota of none is an answer too: the account gets no fee-free transactions at all.
  const count = (n: unknown): n is number => typeof n === "number" && Number.isSafeInteger(n) && n >= 0;
  const named = (v: unknown): v is string => typeof v === "string" && v !== "";
  const error = named(answer.code) ? answer.code : named(answer.error) ? answer.error : undefined;
  return {
    ...(error != null ? { error } : {}),
    ...(positive(answer.retryAt) ? { retryAt: answer.retryAt } : {}),
    ...(count(answer.quota) ? { quota: answer.quota } : {}),
  };
}

export class GaslessUnavailableError extends Error {
  override name = "GaslessUnavailableError";
  constructor(
    message: string,
    readonly reason: GaslessReason = "unknown",
    /** The relay's own word for the refusal, when it answered one. */
    readonly refusal: RelayRefusal = {},
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

/** Who an `executeMetaTransaction` calldata of the target's style runs as, and the call it runs; null when it is not one. */
export function decodeExecuteMetaTransaction(style: MetaTxStyle, data: Hex): { from: string; functionData: Hex } | null {
  try {
    const abi = style === "functionData" ? OFFCHAIN_META_TX_ABI : NATIVE_META_TX_ABI;
    const { functionName, args } = decodeFunctionData({ abi, data });
    if (functionName !== "executeMetaTransaction") return null;
    return { from: String(args[0]).toLowerCase(), functionData: args[1] as Hex };
  } catch {
    return null;
  }
}

function declined(e: unknown): boolean {
  const err = e as { code?: unknown; message?: string } | null;
  return err?.code === 4001 || err?.code === "ACTION_REJECTED" || /denied|reject|cancel/i.test(err?.message ?? "");
}

/**
 * Which nonce a signature may be at: no lower than `minNonce`, or exactly `exactNonce`. The type takes one or the other; a
 * call that passes both anyway is signed at `exactNonce` if the two agree, and refused (`RangeError`, before anything is
 * read, signed or sent) when `minNonce` is above it.
 */
type NonceChoice =
  /** A nonce already observed on chain: never sign against a staler one (load-balanced RPCs lag). */
  | { minNonce?: bigint; exactNonce?: never }
  /**
   * Sign only at this nonce: when the target's nonce reads as any other, nothing is asked of the wallet
   * (`NonceMovedError`). A replacement that must compete with one signed call at its nonce, and no other, uses it.
   */
  | { exactNonce: bigint; minNonce?: never };

export type RelayOptions = NonceChoice & {
  provider: Eip1193Provider;
  reader: ChainReader;
  target: MetaTxTarget;
  from: string;
  functionData: Hex;
  relayerUrl?: string;
  fetchImpl?: typeof fetch;
  /** Fired once the buyer signed (the prompt is gone), before the relay is called. */
  onSigned?: () => void;
  /**
   * The signed execution, before it goes to the relay: what `relaySignedMetaTransaction` can send again, and the nonce it
   * names. A throw here stops it before anything is sent (the caller could not record it).
   */
  onSignedCall?: (call: SignedCall) => void;
};

/** A meta-transaction signed for one nonce: sent again as is, it can still run at most once. */
export type SignedCall = { to: string; data: Hex; nonce: bigint };

export type Relayed = { txHash: Hex; nonce: bigint };

/** The target's nonce read as `read`, not the `expected` one a signature was asked for: nothing was signed or sent. */
export class NonceMovedError extends Error {
  override name = "NonceMovedError";
  constructor(
    readonly expected: bigint,
    readonly read: bigint,
  ) {
    super(`The nonce reads ${read}, not ${expected}`);
  }
}

export async function relayMetaTransaction(opts: RelayOptions): Promise<Relayed> {
  const { provider, reader, target, from, functionData } = opts;
  // The type keeps the two apart; a caller that passes both anyway is refused here when they can't both hold.
  const { exactNonce, minNonce } = opts;
  if (exactNonce !== undefined && minNonce !== undefined && minNonce > exactNonce) {
    throw new RangeError(`A signature can't be at nonce ${exactNonce} and at least ${minNonce}`);
  }
  const code = await reader.code(target.chainId, from).catch(() => "0x" as Hex);
  if (code && code !== "0x") throw new GaslessUnavailableError("Contract accounts are not supported", "contract-account");

  const read = await readMetaTxNonce(reader, target, from);
  if (exactNonce !== undefined && read !== exactNonce) throw new NonceMovedError(exactNonce, read);
  const nonce = exactNonce ?? (minNonce !== undefined && minNonce > read ? minNonce : read);
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
  opts.onSignedCall?.({ to: target.address, data, nonce });
  const txHash = await relaySignedMetaTransaction({ from, to: target.address, data, relayerUrl: opts.relayerUrl, fetchImpl: opts.fetchImpl });
  return { txHash, nonce };
}

/**
 * Sends an execution already signed (`encodeExecuteMetaTransaction`'s call data) through the relay: its hash, or a
 * GaslessUnavailableError, "relayer-rejected" only when the relay says nothing was broadcast (refusedBeforeBroadcast).
 * Sending the same signed execution again can run it at most once: its signature names one nonce.
 */
export async function relaySignedMetaTransaction(opts: { from: string; to: string; data: Hex; relayerUrl?: string; fetchImpl?: typeof fetch }): Promise<Hex> {
  const { from, data } = opts;
  const base = (opts.relayerUrl ?? configuredRelayer()).replace(/\/$/, "");
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  type RelayerBody = { ok?: boolean; txHash?: string; message?: string; code?: unknown; error?: unknown; not_sent?: unknown };
  let body: RelayerBody | null = null;
  let status = 0;
  try {
    const res = await fetchImpl(`${base}/transactions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ transactionData: { from, params: [opts.to, data] } }),
    });
    status = res.status;
    body = (await res.json().catch(() => null)) as RelayerBody | null;
  } catch (e) {
    throw new GaslessUnavailableError((e as Error)?.message ?? "relayer unreachable", "relayer-unreachable");
  }
  if (!body) throw new GaslessUnavailableError(`relayer answered ${status} with no body`, "relayer-unreachable");
  if (body.ok !== false && body.txHash) return body.txHash as Hex;
  const reason = refusedBeforeBroadcast(body) ? "relayer-rejected" : "relayer-unreachable";
  throw new GaslessUnavailableError(body.message ?? `relayer ${status}`, reason, relayRefusal(body));
}

/**
 * Whether a relay's failure proves nothing was broadcast, so the credit can be given back and the
 * buyer may sign again: only when the relay says so explicitly, `{"ok":false,"not_sent":true}`. The
 * relay marks only the failures it answered before any send, or after a send attempt that proves
 * nothing went out. No status is evidence by itself: a relay of another version, or another relay
 * this client is pointed at, can answer a 4xx or a 500 after it sent the transaction.
 */
export function refusedBeforeBroadcast(body: unknown): boolean {
  const answer = body as { ok?: unknown; not_sent?: unknown } | null;
  return answer?.ok === false && answer.not_sent === true;
}

export function isGaslessUnavailable(e: unknown): e is GaslessUnavailableError {
  return e instanceof GaslessUnavailableError || (e as { name?: string } | null)?.name === "GaslessUnavailableError";
}

/** Relay failures that prove nothing was sent: the relay said so (`not_sent`), or it was never asked. */
const NOT_SENT: readonly GaslessReason[] = ["relayer-rejected", "disabled", "contract-account", "signing-failed"];

/**
 * Whether a relayed call is known not to have been sent: only then may the same effect go out another way (the wallet)
 * without risking it twice. Any other failure (a lost answer, an unknown one) may have been sent.
 */
export function provedNotSent(e: unknown): boolean {
  return isGaslessUnavailable(e) && NOT_SENT.includes(e.reason);
}

export function isSignatureDeclined(e: unknown): boolean {
  return e instanceof SignatureDeclinedError || (e as { name?: string } | null)?.name === "SignatureDeclinedError";
}
