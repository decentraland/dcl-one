// from: shop@94584a5c:app/src/lib/authorizations.ts; marketplace@f3956826:webapp/src/components/SettingsPage/SettingsPage.tsx
//       (the MANA/NFT approvals a buyer and seller hold). Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
import { decodeFunctionResult, encodeFunctionData, maxUint256, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "@shop/model/chain";
import type { WalletKind } from "../../wallet/session";
import { COLLECTION_MINTER_ABI, ERC20_ABI, ERC721_ABI } from "@shop/checkout/chain/abi";
import { gaslessEnabled, rpcUrl } from "../chain/config";
import { COLLECTION_V2_DOMAIN, requireContracts } from "../chain/contracts";
import { isSignatureDeclined, provedNotSent, readMetaTxNonce, relayMetaTransaction, relaySignedMetaTransaction, type MetaTxTarget } from "../chain/metatx";
import { shopChainReader, waitForReceipt, type ChainReader } from "../chain/rpc";
import { sendAndConfirm, TxPendingError, TxRevertedError } from "../chain/wallet";
import { activeChainId } from "./network";
import { pageLocks, webLock } from "./web-lock";
import { canPayGasItself, showsWalletConfirmations } from "./wallet-kind";

export enum AuthorizationKind {
  Allowance = "allowance",
  Approval = "approval",
  Minter = "minter",
}

export type ShopAuthorization = {
  kind: AuthorizationKind;
  contractAddress: string;
  spenderAddress: string;
  chainId: ChainId;
};

export type ShopAuthorizationDescriptor = ShopAuthorization & { id: string; group: "buying" | "selling" | "minting" | "bidding" | "renting" };

export function readProvider(): ChainReader {
  return shopChainReader();
}

export async function getAuthorizationStatus(auth: ShopAuthorization, owner: string, requiredWei?: bigint, reader: ChainReader = readProvider()): Promise<boolean> {
  switch (auth.kind) {
    case AuthorizationKind.Allowance: {
      const data = encodeFunctionData({ abi: ERC20_ABI, functionName: "allowance", args: [owner as Hex, auth.spenderAddress as Hex] });
      const allowance = decodeFunctionResult({ abi: ERC20_ABI, functionName: "allowance", data: await reader.call(auth.chainId, auth.contractAddress, data) });
      return requiredWei == null || requiredWei <= 0n ? allowance > 0n : allowance >= requiredWei;
    }
    case AuthorizationKind.Approval: {
      const data = encodeFunctionData({ abi: ERC721_ABI, functionName: "isApprovedForAll", args: [owner as Hex, auth.spenderAddress as Hex] });
      return decodeFunctionResult({ abi: ERC721_ABI, functionName: "isApprovedForAll", data: await reader.call(auth.chainId, auth.contractAddress, data) });
    }
    case AuthorizationKind.Minter: {
      try {
        const data = encodeFunctionData({ abi: COLLECTION_MINTER_ABI, functionName: "globalMinters", args: [auth.spenderAddress as Hex] });
        return decodeFunctionResult({ abi: COLLECTION_MINTER_ABI, functionName: "globalMinters", data: await reader.call(auth.chainId, auth.contractAddress, data) });
      } catch {
        return false;
      }
    }
  }
}

/**
 * The call that grants or revokes `auth`. An allowance is granted in full (`maxUint256`) unless `amount` names it: an
 * ERC-20 `approve` sets the allowance to that value, it does not add to it.
 */
export function encodeAuthorizationCall(auth: ShopAuthorization, active: boolean, amount?: bigint): Hex {
  switch (auth.kind) {
    case AuthorizationKind.Allowance:
      return encodeFunctionData({ abi: ERC20_ABI, functionName: "approve", args: [auth.spenderAddress as Hex, active ? (amount ?? maxUint256) : 0n] });
    case AuthorizationKind.Approval:
      return encodeFunctionData({ abi: ERC721_ABI, functionName: "setApprovalForAll", args: [auth.spenderAddress as Hex, active] });
    case AuthorizationKind.Minter:
      return encodeFunctionData({ abi: COLLECTION_MINTER_ABI, functionName: "setMinters", args: [[auth.spenderAddress as Hex], [active]] });
  }
}

export function metaTxTargetFor(auth: ShopAuthorization): MetaTxTarget | null {
  if (auth.kind === AuthorizationKind.Allowance) {
    const mana = requireContracts(auth.chainId).mana;
    return mana.metaTx ? { address: mana.address, chainId: auth.chainId, name: mana.name, version: mana.version, metaTx: mana.metaTx } : null;
  }
  return { address: auth.contractAddress.toLowerCase(), chainId: auth.chainId, ...COLLECTION_V2_DOMAIN, metaTx: "functionSignature" };
}

const WALLET_METHODS = new Set(["eth_signTypedData_v4", "eth_signTypedData", "eth_sign", "personal_sign", "eth_requestAccounts", "eth_accounts", "eth_chainId"]);

export function metaTxProviderShim(wallet: Eip1193Provider, chainId: ChainId, fetchImpl: typeof fetch = (...a) => fetch(...a)): Eip1193Provider {
  let id = 1;
  return {
    async request({ method, params }) {
      if (WALLET_METHODS.has(method)) return wallet.request({ method, params });
      const url = rpcUrl(chainId);
      if (!url) throw new Error(`No RPC is configured for chain ${chainId}`);
      const res = await fetchImpl(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: id++, method, params: params ?? [] }),
      });
      const body = (await res.json()) as { result?: unknown; error?: { message?: string } };
      if (body.error) throw new Error(body.error.message ?? `${method} failed`);
      return body.result;
    },
  };
}

export type AuthorizationContext = {
  provider: Eip1193Provider;
  account: string;
  walletKind: WalletKind | null;
  reader?: ChainReader;
  relayerUrl?: string;
  fetchImpl?: typeof fetch;
  /** The browser's Web Locks an exact grant runs under (the page's by default; null: none). */
  locks?: LockManager | null;
  /** The caller already holds the exact grant's lock (`exactGrantLock`): a sale holds it across its own dispatch. */
  grantLockHeld?: boolean;
};

async function onChain(provider: Eip1193Provider, chainId: number): Promise<boolean> {
  try {
    return (await activeChainId(provider)) === chainId;
  } catch {
    return false;
  }
}

/**
 * A finite grant whose relayed approve may still land: the relay lost its answer (or the receipt is not in yet). Granting
 * again from the wallet could land beside it, the purchase spend that allowance, and the late relayed approve restore it;
 * so no grant goes out until it resolves.
 */
export class AuthorizationPendingError extends Error {
  constructor(message = "The MANA approval was sent and is still confirming.") {
    super(message);
    this.name = "AuthorizationPendingError";
  }
}

export function isAuthorizationPending(e: unknown): e is AuthorizationPendingError {
  return e instanceof AuthorizationPendingError;
}

/** Pending for another window: it holds the exact grant's lock, and this one asked the wallet nothing. */
export class GrantInProgressElsewhereError extends AuthorizationPendingError {
  constructor() {
    super("A MANA approval is already in progress in another window. Try again in a moment.");
    this.name = "GrantInProgressElsewhereError";
  }
}

/**
 * How long a retry after a lost answer waits before sending the same signed approve again. It only ever allows that
 * check-and-resend: no time lets a recorded grant go.
 */
export const PENDING_GRANT_HOLD_MS = 60_000;
const PENDING_GRANT_KEY = "dcl-shop-exact-grant:v2:";

/**
 * An exact grant sent and not seen to land, kept for every tab of this browser and across a restart: the execution it
 * signed (sent again as is it can still run at most once, since it names one nonce), the allowance it sets (`amount`, in
 * wei) and the attempt that made it, the only one that changes or clears it.
 */
export type PendingGrant = { at: number; attempt: string; amount: string; to: string; data: Hex; nonce: string };

/** The grant could not be recorded before it left, so it was not sent. */
export class GrantNotRecordedError extends Error {
  constructor() {
    super("Your browser couldn't keep a record of this approval, so it wasn't sent.");
    this.name = "GrantNotRecordedError";
  }
}

/** An exact grant needs what makes it go out once: the relay (its nonce reconciles a lost answer) and Web Locks. */
export class ExactGrantUnavailableError extends Error {
  constructor(message = "The MANA approval can't be sent from here right now, so nothing was sent. Try again later.") {
    super(message);
    this.name = "ExactGrantUnavailableError";
  }
}

const NO_LOCKS = "This browser can't make sure the MANA approval is sent only once, so it wasn't sent.";

function grantStorage(): Storage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

function pendingGrantKey(auth: ShopAuthorization, account: string): string {
  return `${PENDING_GRANT_KEY}${account.toLowerCase()}:${auth.chainId}:${auth.contractAddress.toLowerCase()}:${auth.spenderAddress.toLowerCase()}`;
}

/**
 * The Web Lock an exact grant to `auth` by `account` is reconciled and sent under, in every tab: the approval step's and
 * a sale's alike. A sale takes it before its item's lock.
 */
export function exactGrantLock(auth: ShopAuthorization, account: string): string {
  return `dcl-shop-exact-grant:${account.toLowerCase()}:${auth.chainId}:${auth.contractAddress.toLowerCase()}:${auth.spenderAddress.toLowerCase()}`;
}

const digits = (v: unknown): v is string => typeof v === "string" && /^\d+$/.test(v);

/** The exact grant to `auth` this account sent, from any tab, as recorded; whether it can still run is `pendingGrantOpen`'s. */
export function readPendingGrant(auth: ShopAuthorization, account: string): PendingGrant | null {
  try {
    const raw = grantStorage()?.getItem(pendingGrantKey(auth, account)) ?? null;
    const g = raw ? (JSON.parse(raw) as Record<string, unknown> | null) : null;
    if (!g || typeof g.at !== "number" || typeof g.attempt !== "string" || !digits(g.amount)) return null;
    if (typeof g.to !== "string" || typeof g.data !== "string" || !digits(g.nonce)) return null;
    return { at: g.at, attempt: g.attempt, amount: g.amount, to: g.to, data: g.data as Hex, nonce: g.nonce };
  } catch {
    return null;
  }
}

/** Whether the record was kept (written and read back): a grant is sent only once it is. */
function writePendingGrant(auth: ShopAuthorization, account: string, grant: PendingGrant): boolean {
  try {
    const storage = grantStorage();
    if (!storage) return false;
    const key = pendingGrantKey(auth, account);
    const value = JSON.stringify(grant);
    storage.setItem(key, value);
    return storage.getItem(key) === value;
  } catch {
    return false;
  }
}

/** Clears the record only while it is `grant`'s attempt: another attempt's stays. */
function clearPendingGrant(auth: ShopAuthorization, account: string, grant: PendingGrant | null): void {
  if (!grant) return;
  try {
    if (readPendingGrant(auth, account)?.attempt === grant.attempt) grantStorage()?.removeItem(pendingGrantKey(auth, account));
  } catch {
    // Nothing was stored.
  }
}

/** Whether a recorded grant can still run: while the target's nonce has not passed the one it signed. A failed read is no answer. */
async function pendingGrantOpen(grant: PendingGrant, account: string, reader: ChainReader, target: MetaTxTarget | null): Promise<boolean> {
  try {
    return !target || (await readMetaTxNonce(reader, target, account)) <= BigInt(grant.nonce);
  } catch {
    return true;
  }
}

/**
 * Whether a sale may spend an exact allowance of `amount` now: no exact grant this browser recorded to `auth` can still
 * run (a late one would restore the allowance after the sale), and the allowance covers it. A failed read is a no.
 */
export async function exactAllowanceReady(auth: ShopAuthorization, account: string, amount: bigint, reader: ChainReader = readProvider()): Promise<boolean> {
  const pending = readPendingGrant(auth, account);
  if (pending && (await pendingGrantOpen(pending, account, reader, metaTxTargetFor(auth)))) return false;
  return getAuthorizationStatus(auth, account, amount, reader).catch(() => false);
}

function newGrantAttempt(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Runs `run` under the exact grant's lock, unless the caller holds it: none to take (or no Web Locks) sends nothing. */
async function underGrantLock<T>(opts: AuthorizationContext & { auth: ShopAuthorization }, run: () => Promise<T>): Promise<T> {
  if (opts.grantLockHeld) return run();
  const locks = opts.locks === undefined ? pageLocks() : opts.locks;
  if (!locks) throw new ExactGrantUnavailableError(NO_LOCKS);
  const release = await webLock(locks, exactGrantLock(opts.auth, opts.account));
  if (!release) throw new GrantInProgressElsewhereError();
  try {
    return await run();
  } finally {
    release();
  }
}

/**
 * Grants exactly `amount` (the CreditsManager's gap), never beside another exact grant that may still run: a sale could
 * spend one allowance and a late approve restore it. Everything happens under the grant's Web Lock (`exactGrantLock`),
 * taken before the wallet is asked anything, so two windows never both find no record and both send.
 *
 * A recorded grant is reconciled first, by the target's nonce: past the one it signed it can never run, and its record
 * goes. While it can, nothing new is signed and the allowance does not matter (another grant may cover it, and spending
 * that leaves the recorded one to land after the sale): for the same amount, past `PENDING_GRANT_HOLD_MS`, that same signed
 * execution goes to the relay again (whichever of its sends runs, it runs once); otherwise `AuthorizationPendingError`.
 *
 * A new grant goes through the relay only, recorded before it may leave (one this browser cannot record is not sent,
 * `GrantNotRecordedError`). The wallet never sends one: its approve consumes no meta-transaction nonce, so nothing would
 * reconcile it. Without the relay, or when it proves nothing was sent, nothing goes out (`ExactGrantUnavailableError`, or
 * the relay's own refusal).
 */
async function grantExactly(opts: AuthorizationContext & { auth: ShopAuthorization; amount: bigint }): Promise<void> {
  return underGrantLock(opts, () => grantExactlyHeld(opts));
}

async function grantExactlyHeld(opts: AuthorizationContext & { auth: ShopAuthorization; amount: bigint }): Promise<void> {
  const { auth, amount, provider, account } = opts;
  const reader = opts.reader ?? readProvider();
  const target = metaTxTargetFor(auth);
  const pending = readPendingGrant(auth, account);
  if (pending) {
    if (await pendingGrantOpen(pending, account, reader, target)) {
      if (BigInt(pending.amount) === amount && gaslessEnabled() && Date.now() - pending.at >= PENDING_GRANT_HOLD_MS) return resendPendingGrant(opts, pending, reader);
      throw new AuthorizationPendingError();
    }
    clearPendingGrant(auth, account, pending);
  }
  if (await getAuthorizationStatus(auth, account, amount, reader)) return;
  if (!gaslessEnabled() || !target) throw new ExactGrantUnavailableError();
  const attempt = newGrantAttempt();
  const functionData = encodeAuthorizationCall(auth, true, amount);
  const sent: { grant: PendingGrant | null } = { grant: null };
  try {
    const { txHash } = await relayMetaTransaction({
      provider,
      reader,
      target,
      from: account,
      functionData,
      relayerUrl: opts.relayerUrl,
      fetchImpl: opts.fetchImpl,
      onSignedCall: ({ to, data, nonce }) => {
        // Never over another attempt's record (one written by a window without the lock): this one is not sent.
        if (readPendingGrant(auth, account)) throw new AuthorizationPendingError();
        const grant: PendingGrant = { at: Date.now(), attempt, amount: amount.toString(), to, data, nonce: nonce.toString() };
        if (!writePendingGrant(auth, account, grant)) throw new GrantNotRecordedError();
        sent.grant = grant;
      },
    });
    const receipt = await waitForReceipt(reader, auth.chainId, txHash);
    if (!receipt) throw new TxPendingError(txHash, "The authorization");
    if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The authorization");
    clearPendingGrant(auth, account, sent.grant);
  } catch (e) {
    if (sent.grant && !provedNotSent(e) && !(e instanceof TxRevertedError)) throw e instanceof TxPendingError ? e : new AuthorizationPendingError();
    // Never sent (declined, refused before it left, or as not sent) or reverted: its record goes, and nothing goes in its place.
    clearPendingGrant(auth, account, sent.grant);
    throw e;
  }
}

/** The recorded approve, sent again exactly as signed. Its record goes only once a send of it confirms. */
async function resendPendingGrant(opts: AuthorizationContext & { auth: ShopAuthorization }, grant: PendingGrant, reader: ChainReader): Promise<void> {
  const { auth, account } = opts;
  try {
    const txHash = await relaySignedMetaTransaction({ from: account, to: grant.to, data: grant.data, relayerUrl: opts.relayerUrl, fetchImpl: opts.fetchImpl });
    const receipt = await waitForReceipt(reader, auth.chainId, txHash);
    if (!receipt) throw new TxPendingError(txHash, "The authorization");
    if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The authorization");
    clearPendingGrant(auth, account, grant);
  } catch (e) {
    // Refused, reverted (its nonce may be spent already) or lost: still reconciled by its nonce on the next try.
    throw e instanceof TxPendingError ? e : new AuthorizationPendingError();
  }
}

/** `amount`: an allowance granted at exactly that value instead of in full (see `encodeAuthorizationCall`, `grantExactly`). */
export async function setAuthorization(opts: AuthorizationContext & { auth: ShopAuthorization; active: boolean; amount?: bigint }): Promise<void> {
  const { auth, active, provider, account } = opts;
  if (opts.amount !== undefined && (auth.kind !== AuthorizationKind.Allowance || opts.amount <= 0n || !active)) {
    throw new Error("An exact amount is granted only as a positive MANA allowance");
  }
  if (opts.amount !== undefined) return grantExactly({ ...opts, amount: opts.amount });
  const reader = opts.reader ?? readProvider();
  const functionData = encodeAuthorizationCall(auth, active, opts.amount);
  const target = metaTxTargetFor(auth);
  if (gaslessEnabled() && target) {
    try {
      const { txHash } = await relayMetaTransaction({ provider, reader, target, from: account, functionData, relayerUrl: opts.relayerUrl, fetchImpl: opts.fetchImpl });
      const receipt = await waitForReceipt(reader, auth.chainId, txHash);
      if (!receipt) throw new TxPendingError(txHash, "The authorization");
      if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The authorization");
      return;
    } catch (e) {
      if (isSignatureDeclined(e) || e instanceof TxPendingError) throw e;
      // Unlike a purchase, this falls back to the wallet even when the relay may have sent the
      // call (`relayer-unreachable`): the same change applied twice leaves it applied once, so the
      // cost is a second prompt and its gas, and a web3 buyer can still approve or revoke while the
      // relay is down. The risk taken: the relayed call may still land later, after the wallet sent
      // the opposite change meanwhile (the wallet's own transaction does not consume the
      // meta-transaction nonce). A late relayed grant turns back on an authorization revoked since,
      // and a late relayed revoke turns off one granted again since. Accepted, not handled.
      const fallback = canPayGasItself(opts.walletKind) && (await onChain(provider, auth.chainId));
      if (!fallback) throw e;
    }
  }
  await sendAndConfirm({ provider, reader, chainId: auth.chainId, from: account, to: auth.contractAddress, data: functionData, what: "The authorization" });
}

/**
 * Grants `auth` unless it already covers `requiredWei`. With `exact`, an allowance short of `requiredWei` is set to exactly
 * `requiredWei`, never more (the spender that pulls a credits + MANA gap), and only once no recorded exact grant can still
 * run (`grantExactly`); otherwise it is granted in full.
 */
export async function ensureAuthorization(opts: AuthorizationContext & { auth: ShopAuthorization; requiredWei?: bigint; exact?: boolean }): Promise<void> {
  if (opts.exact && (opts.requiredWei === undefined || opts.requiredWei <= 0n)) {
    throw new Error("An exact allowance needs a positive amount");
  }
  const { exact, requiredWei, ...rest } = opts;
  // Exact: a recorded grant is reconciled before the allowance is read (one another grant covers does not retire it).
  if (exact) return grantExactly({ ...rest, amount: requiredWei! });
  const authorized = await getAuthorizationStatus(opts.auth, opts.account, requiredWei, opts.reader ?? readProvider());
  if (authorized) return;
  await setAuthorization({ ...rest, active: true });
}

export function needsApprovalStep(walletKind: WalletKind | null | undefined, isAuthorized: boolean): boolean {
  return showsWalletConfirmations(walletKind) && !isAuthorized;
}

export function getManaSpendingAuthorization(chainId: ChainId, spenderAddress: string): ShopAuthorization {
  return { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spenderAddress.toLowerCase(), chainId };
}

