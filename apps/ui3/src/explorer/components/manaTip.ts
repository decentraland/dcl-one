import { encodeFunctionData, isAddress, parseUnits, toHex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { CatalystError, getJSON, sendSignedJSON, type SendOpts } from "../../data/catalyst/client";
import type { PaymentsConfig } from "@catalyst-api/economy/PaymentsConfig";

const MANA_DECIMALS = 18;
export const PAYMENTS_CONFIG_PATH = "/v1/payments/config";
export const TIP_FORWARD_PATH = "/events-notifier/forward-blockchain";
const TIP_FORWARD_TIMEOUT_MS = 10_000;
// How long a tip keeps waiting to be mined and forwarded while the page is open.
export const TIP_DELIVERY_WINDOW_MS = 15 * 60_000;
const RECEIPT_POLL_MS = 4_000;
const FORWARD_FIRST_RETRY_MS = 5_000;
const FORWARD_MAX_RETRY_MS = 120_000;
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

export type TipDelivery = "notified" | "reverted" | "refused" | "gave-up";

export type TipDeliveryOpts = {
  forward?: (transactionHash: string) => Promise<void>;
  sleep?: (ms: number) => Promise<void>;
  now?: () => number;
  windowMs?: number;
};

const realSleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

// 429, 5xx and network failures (status 0) may succeed later.
function retryable(e: unknown): boolean {
  const status = e instanceof CatalystError ? e.status : 0;
  return status === 0 || status === 429 || status >= 500;
}

/**
 * Waits for the tip's receipt through the wallet, then forwards it, retrying
 * retryable failures with backoff until the window closes. Never throws.
 */
export async function deliverTip(provider: Eip1193Provider, hash: string, opts: TipDeliveryOpts = {}): Promise<TipDelivery> {
  const forward = opts.forward ?? ((tx: string) => forwardTip(tx));
  const sleep = opts.sleep ?? realSleep;
  const now = opts.now ?? Date.now;
  const deadline = now() + (opts.windowMs ?? TIP_DELIVERY_WINDOW_MS);
  for (;;) {
    let status: unknown;
    try {
      const receipt = (await provider.request({ method: "eth_getTransactionReceipt", params: [hash] })) as { status?: unknown } | null;
      status = receipt?.status;
    } catch {
      // A failed read is not a revert: keep polling.
    }
    if (status === "0x1") break;
    if (status === "0x0") return "reverted";
    if (now() + RECEIPT_POLL_MS > deadline) return "gave-up";
    await sleep(RECEIPT_POLL_MS);
  }
  let delay = FORWARD_FIRST_RETRY_MS;
  for (;;) {
    let hint = 0;
    try {
      await forward(hash);
      return "notified";
    } catch (e) {
      if (!retryable(e)) return "refused";
      hint = e instanceof CatalystError ? (e.retryAfterSecs ?? 0) * 1000 : 0;
    }
    // Never sooner than the server's Retry-After, never longer than the max.
    const wait = Math.min(Math.max(delay, hint), FORWARD_MAX_RETRY_MS);
    if (now() + wait > deadline) return "gave-up";
    await sleep(wait);
    delay = Math.min(delay * 2, FORWARD_MAX_RETRY_MS);
  }
}
