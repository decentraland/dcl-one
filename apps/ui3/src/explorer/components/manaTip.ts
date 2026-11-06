import { encodeFunctionData, isAddress, parseUnits, toHex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { getJSON, sendSignedJSON, type SendOpts } from "../../data/catalyst/client";
import type { PaymentsConfig } from "@catalyst-api/economy/PaymentsConfig";

const MANA_DECIMALS = 18;
export const PAYMENTS_CONFIG_PATH = "/v1/payments/config";
export const TIP_FORWARD_PATH = "/events-notifier/forward-blockchain";
const TIP_FORWARD_TIMEOUT_MS = 10_000;
const POLYGON_CHAIN_IDS = new Set([137, 80002]);
const TX_HASH_RE = /^0x[0-9a-fA-F]{64}$/;

const TRANSFER_ABI = [
  {
    type: "function",
    name: "transfer",
    stateMutability: "nonpayable",
    inputs: [
      { name: "to", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    outputs: [{ name: "", type: "bool" }],
  },
] as const;

const AMOUNT_RE = /^\d+(\.\d+)?$/;

export function manaToWei(amount: string): bigint | null {
  const trimmed = amount.trim();
  if (!AMOUNT_RE.test(trimmed)) return null;
  try {
    const wei = parseUnits(trimmed, MANA_DECIMALS);
    return wei > 0n ? wei : null;
  } catch {
    return null;
  }
}

export function encodeManaTransfer(to: string, wei: bigint): `0x${string}` {
  if (!isAddress(to)) throw new Error("The recipient is not a valid wallet address.");
  return encodeFunctionData({ abi: TRANSFER_ABI, functionName: "transfer", args: [to, wei] });
}

export type TipTarget = { chainId: number; manaToken: string };

export function tipTargetFromConfig(cfg: unknown): TipTarget | null {
  const c = cfg as Partial<PaymentsConfig> | null;
  if (!c || typeof c.chainId !== "number" || !Number.isInteger(c.chainId) || c.chainId <= 0) return null;
  if (typeof c.manaToken !== "string" || !isAddress(c.manaToken)) return null;
  return { chainId: c.chainId, manaToken: c.manaToken };
}

export async function fetchTipTarget(signal?: AbortSignal): Promise<TipTarget | null> {
  return tipTargetFromConfig(await getJSON<unknown>(PAYMENTS_CONFIG_PATH, { signal }));
}

export function chainName(chainId: number): string {
  switch (chainId) {
    case 1:
      return "Ethereum";
    case 137:
      return "Polygon";
    case 11155111:
      return "Sepolia";
    case 80002:
      return "Polygon Amoy";
    default:
      return `chain ${chainId}`;
  }
}

function sameChain(have: unknown, wantHex: string): boolean {
  try {
    return BigInt(String(have)) === BigInt(wantHex);
  } catch {
    return false;
  }
}

type SendManaTipOpts = { from: string; to: string; wei: bigint; target: TipTarget };

export async function sendManaTip(provider: Eip1193Provider, opts: SendManaTipOpts): Promise<string> {
  const data = encodeManaTransfer(opts.to, opts.wei);
  const wantHex = toHex(opts.target.chainId);
  const have = await provider.request({ method: "eth_chainId" });
  if (!sameChain(have, wantHex)) {
    await provider.request({
      method: "wallet_switchEthereumChain",
      params: [{ chainId: wantHex }],
    });
  }
  const hash = await provider.request({
    method: "eth_sendTransaction",
    params: [{ from: opts.from, to: opts.target.manaToken, data }],
  });
  if (typeof hash !== "string" || !hash.startsWith("0x")) {
    throw new Error("The wallet returned no transaction hash.");
  }
  return hash;
}

// The server only verifies Polygon MANA transfers, signed by the tipping wallet.
export function shouldForwardTip(opts: { target: TipTarget; hash: string; from: string; signer?: string | null }): boolean {
  return POLYGON_CHAIN_IDS.has(opts.target.chainId)
    && TX_HASH_RE.test(opts.hash)
    && !!opts.signer
    && opts.signer.toLowerCase() === opts.from.toLowerCase();
}

export async function forwardTip(transactionHash: string, opts: SendOpts = {}): Promise<void> {
  await sendSignedJSON(TIP_FORWARD_PATH, {
    timeoutMs: TIP_FORWARD_TIMEOUT_MS,
    ...opts,
    method: "POST",
    body: { type: "tip-received", transactionHash },
  });
}
