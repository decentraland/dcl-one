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
import { isWalletRefusal, sendAndConfirm, TxPendingError, TxRevertedError } from "../chain/wallet";
import { activeChainId } from "./network";
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

/** How long a retry after a lost answer waits for it to land before sending the same signed approve again. */
export const PENDING_GRANT_HOLD_MS = 60_000;
/** When a grant the wallet lost the answer to, with no hash to reconcile it by, stops holding further grants. */
const LOST_WALLET_GRANT_MAX_MS = 24 * 60 * 60 * 1000;
const PENDING_GRANT_KEY = "dcl-shop-exact-grant:v2:";

/**
 * An exact grant sent and not seen to land, kept for every tab of this browser and across a restart: a relayed one as the
 * execution it signed (sent again as is it can still run at most once, since it names one nonce), one handed to the
 * wallet by the hash the wallet gave back, if it gave one.
 */
export type PendingGrant = { at: number; via: "relay"; to: string; data: Hex; nonce: string } | { at: number; via: "wallet"; txHash: string | null };

/** The grant could not be recorded before it left, so it was not sent. */
export class GrantNotRecordedError extends Error {
  constructor() {
    super("Your browser couldn't keep a record of this approval, so it wasn't sent.");
    this.name = "GrantNotRecordedError";
  }
}

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

function readStoredGrant(auth: ShopAuthorization, account: string): PendingGrant | null {
  try {
    const raw = grantStorage()?.getItem(pendingGrantKey(auth, account)) ?? null;
    const g = raw ? (JSON.parse(raw) as Record<string, unknown> | null) : null;
    if (!g || typeof g.at !== "number") return null;
    if (g.via === "relay" && typeof g.to === "string" && typeof g.data === "string" && typeof g.nonce === "string" && /^\d+$/.test(g.nonce)) {
      return { at: g.at, via: "relay", to: g.to, data: g.data as Hex, nonce: g.nonce };
    }
    if (g.via === "wallet") return { at: g.at, via: "wallet", txHash: typeof g.txHash === "string" ? g.txHash : null };
    return null;
  } catch {
    return null;
  }
}

/**
 * The exact grant to `auth` this account sent, from any tab, that may still land. One the wallet lost the answer to, with
 * no hash, has nothing to reconcile it by: it holds for a day.
 */
export function readPendingGrant(auth: ShopAuthorization, account: string, now: number = Date.now()): PendingGrant | null {
  const grant = readStoredGrant(auth, account);
  if (grant?.via === "wallet" && !grant.txHash && (now < grant.at || now - grant.at > LOST_WALLET_GRANT_MAX_MS)) return null;
  return grant;
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

/** Clears the record only while it is `grant`: another tab's newer one stays. */
function clearPendingGrant(auth: ShopAuthorization, account: string, grant: PendingGrant | null): void {
  if (!grant) return;
  try {
    const stored = readStoredGrant(auth, account);
    if (stored?.at === grant.at && stored.via === grant.via) grantStorage()?.removeItem(pendingGrantKey(auth, account));
  } catch {
    // Nothing was stored.
  }
}

/**
 * Whether a recorded grant can still land: a relayed one while the target's nonce has not passed the one it signed; one
 * handed to the wallet while its receipt is not in, or, with no hash, while it is recorded. A failed read is no answer.
 */
async function pendingGrantOpen(grant: PendingGrant, auth: ShopAuthorization, account: string, reader: ChainReader, target: MetaTxTarget | null): Promise<boolean> {
  try {
    if (grant.via === "relay") return !target || (await readMetaTxNonce(reader, target, account)) <= BigInt(grant.nonce);
    return !grant.txHash || (await reader.receipt(auth.chainId, grant.txHash)) === null;
  } catch {
    return true;
  }
}

/**
 * Grants exactly `amount` (the CreditsManager's gap), never beside another exact grant that may still land: the purchase
 * could spend one allowance and a late approve restore it. So every grant is recorded, for every tab, before it may
 * leave, and one this browser cannot record is not sent (`GrantNotRecordedError`).
 *
 * A recorded one is reconciled first. A relayed one by the target's nonce, read before the allowance: past the nonce it
 * signed, it can never run, and the allowance decides whether another is needed; at it, a covered allowance is enough,
 * and otherwise nothing new is signed: within `PENDING_GRANT_HOLD_MS` nothing is sent, after it the same signed execution
 * goes to the relay again, so whichever of its sends runs, it runs once. One handed to the wallet by its receipt; with no
 * hash, it holds (`readPendingGrant`). While one is open, the caller gets `AuthorizationPendingError`.
 *
 * A new grant goes through the relay; the wallet sends it only when the relay proved nothing was sent (or it reverted).
 * Handed to the wallet, only the buyer's refusal or a mined revert clear its record: a wallet that loses the answer after
 * broadcasting leaves it pending.
 */
async function grantExactly(opts: AuthorizationContext & { auth: ShopAuthorization; amount: bigint }): Promise<void> {
  const { auth, amount, provider, account } = opts;
  const reader = opts.reader ?? readProvider();
  const target = metaTxTargetFor(auth);
  const pending = readPendingGrant(auth, account);
  if (pending) {
    if (await pendingGrantOpen(pending, auth, account, reader, target)) {
      if (await getAuthorizationStatus(auth, account, amount, reader)) return;
      if (pending.via === "relay" && gaslessEnabled() && Date.now() - pending.at >= PENDING_GRANT_HOLD_MS) return resendPendingGrant(opts, pending, reader);
      throw new AuthorizationPendingError();
    }
    clearPendingGrant(auth, account, pending);
    if (await getAuthorizationStatus(auth, account, amount, reader)) return;
  }
  const functionData = encodeAuthorizationCall(auth, true, amount);
  if (gaslessEnabled() && target) {
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
          const grant: PendingGrant = { at: Date.now(), via: "relay", to, data, nonce: nonce.toString() };
          if (!writePendingGrant(auth, account, grant)) throw new GrantNotRecordedError();
          sent.grant = grant;
        },
      });
      const receipt = await waitForReceipt(reader, auth.chainId, txHash);
      if (!receipt) throw new TxPendingError(txHash, "The authorization");
      if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The authorization");
      clearPendingGrant(auth, account, sent.grant);
      return;
    } catch (e) {
      if (e instanceof GrantNotRecordedError || isSignatureDeclined(e)) throw e;
      if (sent.grant && !provedNotSent(e) && !(e instanceof TxRevertedError)) throw e instanceof TxPendingError ? e : new AuthorizationPendingError();
      // Never sent (failed before it reached the relay, or refused as not sent) or reverted: the wallet may grant instead.
      clearPendingGrant(auth, account, sent.grant);
      if (!canPayGasItself(opts.walletKind) || !(await onChain(provider, auth.chainId))) throw e;
    }
  }
  // The wallet's approve consumes no meta-transaction nonce: it is never sent while another tab's grant may still land.
  if (readPendingGrant(auth, account)) throw new AuthorizationPendingError();
  const handed: { grant: PendingGrant | null } = { grant: null };
  try {
    await sendAndConfirm({
      provider,
      reader,
      chainId: auth.chainId,
      from: account,
      to: auth.contractAddress,
      data: functionData,
      what: "The authorization",
      onHandoff: () => {
        const grant: PendingGrant = { at: Date.now(), via: "wallet", txHash: null };
        if (!writePendingGrant(auth, account, grant)) throw new GrantNotRecordedError();
        handed.grant = grant;
      },
      onBroadcast: (txHash) => {
        const grant: PendingGrant = { at: Date.now(), via: "wallet", txHash };
        if (writePendingGrant(auth, account, grant)) handed.grant = grant;
      },
    });
    clearPendingGrant(auth, account, handed.grant);
  } catch (e) {
    if (!handed.grant) throw e;
    if (isWalletRefusal(e) || e instanceof TxRevertedError) {
      clearPendingGrant(auth, account, handed.grant);
      throw e;
    }
    throw e instanceof TxPendingError ? e : new AuthorizationPendingError();
  }
}

/** The recorded relayed approve, sent again exactly as signed. Its record goes only once a send of it confirms. */
async function resendPendingGrant(opts: AuthorizationContext & { auth: ShopAuthorization }, grant: Extract<PendingGrant, { via: "relay" }>, reader: ChainReader): Promise<void> {
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
 * `requiredWei`, never more (the spender that pulls a credits + MANA gap); otherwise it is granted in full.
 */
export async function ensureAuthorization(opts: AuthorizationContext & { auth: ShopAuthorization; requiredWei?: bigint; exact?: boolean }): Promise<void> {
  if (opts.exact && (opts.requiredWei === undefined || opts.requiredWei <= 0n)) {
    throw new Error("An exact allowance needs a positive amount");
  }
  const authorized = await getAuthorizationStatus(opts.auth, opts.account, opts.requiredWei, opts.reader ?? readProvider());
  if (authorized) return;
  const { exact, requiredWei, ...rest } = opts;
  await setAuthorization({ ...rest, active: true, ...(exact ? { amount: requiredWei } : {}) });
}

export function needsApprovalStep(walletKind: WalletKind | null | undefined, isAuthorized: boolean): boolean {
  return showsWalletConfirmations(walletKind) && !isAuthorized;
}

export function getManaSpendingAuthorization(chainId: ChainId, spenderAddress: string): ShopAuthorization {
  return { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spenderAddress.toLowerCase(), chainId };
}

