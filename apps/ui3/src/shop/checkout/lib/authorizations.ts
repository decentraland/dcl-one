// from: shop@94584a5c:app/src/lib/authorizations.ts; marketplace@f3956826:webapp/src/components/SettingsPage/SettingsPage.tsx
//       (the MANA/NFT approvals a buyer and seller hold). Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
import { decodeFunctionResult, encodeFunctionData, maxUint256, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "@shop/model/chain";
import type { WalletKind } from "../../wallet/session";
import { COLLECTION_MINTER_ABI, ERC20_ABI, ERC721_ABI } from "@shop/checkout/chain/abi";
import { gaslessEnabled, rpcUrl } from "../chain/config";
import { COLLECTION_V2_DOMAIN, requireContracts } from "../chain/contracts";
import { isSignatureDeclined, provedNotSent, relayMetaTransaction, type MetaTxTarget } from "../chain/metatx";
import { shopChainReader, waitForReceipt, type ChainReader } from "../chain/rpc";
import { sendAndConfirm, TxPendingError, TxRevertedError } from "../chain/wallet";
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

/** How long a retry re-reads instead of granting again after a lost answer; and when a record is too old to matter. */
export const PENDING_GRANT_HOLD_MS = 60_000;
const PENDING_GRANT_MAX_MS = 24 * 60 * 60 * 1000;
const PENDING_GRANT_KEY = "dcl-shop-exact-grant:v1:";

type PendingGrant = { at: number };

function pendingGrantKey(auth: ShopAuthorization, account: string): string {
  return `${PENDING_GRANT_KEY}${account.toLowerCase()}:${auth.chainId}:${auth.contractAddress.toLowerCase()}:${auth.spenderAddress.toLowerCase()}`;
}

/** The finite grant to `auth` this account sent and has not seen land, if any (this tab). */
export function readPendingGrant(auth: ShopAuthorization, account: string, now: number = Date.now()): PendingGrant | null {
  try {
    const raw = sessionStorage.getItem(pendingGrantKey(auth, account));
    const parsed = raw ? (JSON.parse(raw) as Partial<PendingGrant>) : null;
    if (!parsed || typeof parsed.at !== "number" || now < parsed.at || now - parsed.at > PENDING_GRANT_MAX_MS) return null;
    return { at: parsed.at };
  } catch {
    return null;
  }
}

function writePendingGrant(auth: ShopAuthorization, account: string): void {
  try {
    sessionStorage.setItem(pendingGrantKey(auth, account), JSON.stringify({ at: Date.now() }));
  } catch {
    // Without storage the hold lasts as long as the error the caller shows.
  }
}

function clearPendingGrant(auth: ShopAuthorization, account: string): void {
  try {
    sessionStorage.removeItem(pendingGrantKey(auth, account));
  } catch {
    // Nothing was stored.
  }
}

/**
 * Grants exactly `amount` (the CreditsManager's gap). Unlike a full grant, a relayed approve whose answer was lost is never
 * doubled from the wallet: it is recorded as pending and refused (`AuthorizationPendingError`). A retry then re-reads the
 * allowance first (an approve that landed meanwhile is enough); within `PENDING_GRANT_HOLD_MS` it grants nothing; later it
 * grants again only through the relay, whose meta-transaction reuses the pending one's nonce while that one has not run, so
 * at most one of them can ever land. The record goes only with a relayed grant that confirmed (it used that nonce): an
 * allowance seen covered may be another grant's, and the lost one could still land after a purchase spends it. The wallet
 * sends it only when the relay proved nothing was sent (or it reverted), and never while a lost one is pending.
 */
async function grantExactly(opts: AuthorizationContext & { auth: ShopAuthorization; amount: bigint }): Promise<void> {
  const { auth, amount, provider, account } = opts;
  const reader = opts.reader ?? readProvider();
  const pending = readPendingGrant(auth, account);
  if (pending) {
    if (await getAuthorizationStatus(auth, account, amount, reader)) return;
    if (Date.now() - pending.at < PENDING_GRANT_HOLD_MS) throw new AuthorizationPendingError();
  }
  const functionData = encodeAuthorizationCall(auth, true, amount);
  const target = metaTxTargetFor(auth);
  if (gaslessEnabled() && target) {
    try {
      const { txHash } = await relayMetaTransaction({ provider, reader, target, from: account, functionData, relayerUrl: opts.relayerUrl, fetchImpl: opts.fetchImpl });
      const receipt = await waitForReceipt(reader, auth.chainId, txHash);
      if (!receipt) throw new TxPendingError(txHash, "The authorization");
      if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The authorization");
      clearPendingGrant(auth, account);
      return;
    } catch (e) {
      if (isSignatureDeclined(e)) throw e;
      const notSent = provedNotSent(e) || e instanceof TxRevertedError;
      if (!notSent) {
        writePendingGrant(auth, account);
        throw e instanceof TxPendingError ? e : new AuthorizationPendingError();
      }
      // After a lost answer only the relay grants (above); the wallet's approve would not consume its nonce.
      if (pending || !canPayGasItself(opts.walletKind) || !(await onChain(provider, auth.chainId))) throw e;
    }
  } else if (pending) {
    throw new AuthorizationPendingError();
  }
  await sendAndConfirm({ provider, reader, chainId: auth.chainId, from: account, to: auth.contractAddress, data: functionData, what: "The authorization" });
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

