// from: shop@94584a5c:app/src/lib/authorizations.ts; marketplace@f3956826:webapp/src/components/SettingsPage/SettingsPage.tsx
//       (the MANA/NFT approvals a buyer and seller hold). Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
import { decodeFunctionResult, encodeFunctionData, maxUint256, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import type { WalletKind } from "../../wallet/session";
import { COLLECTION_MINTER_ABI, ERC20_ABI, ERC721_ABI } from "../chain/abi";
import { gaslessEnabled, rpcUrl } from "../chain/config";
import { COLLECTION_V2_DOMAIN, requireContracts } from "../chain/contracts";
import { isSignatureDeclined, relayMetaTransaction, type MetaTxTarget } from "../chain/metatx";
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

export function encodeAuthorizationCall(auth: ShopAuthorization, active: boolean): Hex {
  switch (auth.kind) {
    case AuthorizationKind.Allowance:
      return encodeFunctionData({ abi: ERC20_ABI, functionName: "approve", args: [auth.spenderAddress as Hex, active ? maxUint256 : 0n] });
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

export async function setAuthorization(opts: AuthorizationContext & { auth: ShopAuthorization; active: boolean }): Promise<void> {
  const { auth, active, provider, account } = opts;
  const reader = opts.reader ?? readProvider();
  const functionData = encodeAuthorizationCall(auth, active);
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
      const fallback = canPayGasItself(opts.walletKind) && (await onChain(provider, auth.chainId));
      if (!fallback) throw e;
    }
  }
  await sendAndConfirm({ provider, reader, chainId: auth.chainId, from: account, to: auth.contractAddress, data: functionData, what: "The authorization" });
}

export async function ensureAuthorization(opts: AuthorizationContext & { auth: ShopAuthorization; requiredWei?: bigint }): Promise<void> {
  const authorized = await getAuthorizationStatus(opts.auth, opts.account, opts.requiredWei, opts.reader ?? readProvider());
  if (authorized) return;
  await setAuthorization({ ...opts, active: true });
}

export function needsApprovalStep(walletKind: WalletKind | null | undefined, isAuthorized: boolean): boolean {
  return showsWalletConfirmations(walletKind) && !isAuthorized;
}

export function getManaSpendingAuthorization(chainId: ChainId, spenderAddress: string): ShopAuthorization {
  return { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spenderAddress.toLowerCase(), chainId };
}

