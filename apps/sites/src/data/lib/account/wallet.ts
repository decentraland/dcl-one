import { manaToken } from "@shop/market/lib/contracts";
import { CHAIN } from "@shop/model/chain";
import { formatMana, manaWei } from "@shop/model/money";
import { z } from "zod";

import { browserProvider, web3Usable } from "../auth/shop-wallet";
import type { AuthIdentity } from "../auth/types";
import type { Eip1193Provider } from "../auth/wallet";
import { CatalystError, getJSON, postPublicJSON } from "../catalyst/client";

/**
 * The account's MANA on the two networks the wallets section shows. Every read is one the node already
 * serves, or one the member's own wallet answers without a prompt; nothing here signs or sends.
 */
export type WalletNetwork = "ethereum" | "polygon";

/** `relay` is the node's JSON-RPC relay name (`POST /rpc/{network}`), `index` its transfer index's. */
export const WALLET_NETWORKS = {
  ethereum: { chainId: CHAIN.ETHEREUM, relay: "mainnet", index: "ethereum" },
  polygon: { chainId: CHAIN.POLYGON, relay: "polygon", index: "matic" },
} as const satisfies Record<WalletNetwork, { chainId: number; relay: string; index: string }>;

type EthCall = { to: string; data: string };
type OwnWallet = Pick<Eip1193Provider, "request">;

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const BALANCE_OF = "0x70a08231";

function balanceOfCall(account: string, network: WalletNetwork): EthCall {
  if (!ADDRESS.test(account)) throw new RangeError("Not a wallet address");
  const owner = account.slice(2).toLowerCase().padStart(64, "0");
  return { to: manaToken(WALLET_NETWORKS[network].chainId).address, data: `${BALANCE_OF}${owner}` };
}

/** A `uint256` reply as decimal wei. An empty reply (`0x`: no MANA contract answered) is a failed read, not zero. */
function weiOf(result: unknown): string {
  if (typeof result !== "string" || !/^0x[0-9a-fA-F]{1,64}$/.test(result)) {
    throw new Error("The balance read returned no amount");
  }
  return BigInt(result).toString();
}

const RpcReplySchema = z.object({
  result: z.unknown().optional(),
  error: z.object({ message: z.string().optional() }).nullish(),
});

/** The relay answers a network it has no upstream for with a JSON-RPC error, not an HTTP one. */
async function readThroughRelay(network: WalletNetwork, call: EthCall, signal?: AbortSignal): Promise<string> {
  const raw = await postPublicJSON(
    `/rpc/${WALLET_NETWORKS[network].relay}`,
    { jsonrpc: "2.0", id: 1, method: "eth_call", params: [call, "latest"] },
    { signal },
  );
  const reply = RpcReplySchema.parse(raw);
  if (reply.error) throw new Error(reply.error.message ?? "The balance read failed");
  return weiOf(reply.result);
}

/**
 * The same read through the member's own wallet. It answers only for the account it lists and the
 * chain it is on, and it is never asked to connect or to switch: each request here is silent.
 */
async function readThroughOwnWallet(wallet: OwnWallet, account: string, network: WalletNetwork, call: EthCall) {
  const accounts = await wallet.request({ method: "eth_accounts" });
  const listed =
    Array.isArray(accounts) &&
    accounts.some((entry) => typeof entry === "string" && entry.toLowerCase() === account.toLowerCase());
  if (!listed) throw new Error("The wallet does not list this account");
  const chain = await wallet.request({ method: "eth_chainId" });
  if (Number.parseInt(String(chain), 16) !== WALLET_NETWORKS[network].chainId) {
    throw new Error("The wallet is on another network");
  }
  return weiOf(await wallet.request({ method: "eth_call", params: [call, "latest"] }));
}

/** The browser wallet of a recorded web3 sign-in as `identity`; null for a managed session or none here. */
export function ownWalletOf(identity: Pick<AuthIdentity, "signer">): OwnWallet | null {
  return web3Usable(identity) ? browserProvider() : null;
}

type BalanceOptions = { signal?: AbortSignal; ownWallet?: () => OwnWallet | null };

/**
 * `account`'s MANA on `network`, in wei as a decimal string. The node's relay is read first; where it
 * has no upstream for the network the member's own wallet is asked, when one is given. With neither the
 * relay's failure is thrown: the caller shows the balance as not available, never as zero.
 */
export async function fetchManaBalance(
  account: string,
  network: WalletNetwork,
  opts: BalanceOptions = {},
): Promise<string> {
  const call = balanceOfCall(account, network);
  try {
    return await readThroughRelay(network, call, opts.signal);
  } catch (relayError) {
    if (opts.signal?.aborted) throw relayError;
    const wallet = opts.ownWallet?.();
    if (!wallet) throw relayError;
    try {
      return await readThroughOwnWallet(wallet, account, network, call);
    } catch {
      throw relayError;
    }
  }
}

const HASH = /^0x[0-9a-fA-F]{64}$/;
const index = z.number().int().nonnegative();
const TransferSchema = z.object({
  blockNumber: index,
  logIndex: index,
  transactionHash: z.string().regex(HASH),
  from: z.string().regex(ADDRESS),
  to: z.string().regex(ADDRESS),
  amount: z.string().regex(/^\d+$/),
  timestamp: index,
});
const TransfersCursorSchema = z.object({ beforeBlock: index, beforeLog: index });
const TransfersPageSchema = z.object({
  data: z.array(TransferSchema),
  next: TransfersCursorSchema.nullish(),
});

export type ManaTransfersCursor = z.infer<typeof TransfersCursorSchema>;

/** One confirmed MANA transfer the account sent or received; `amount` is wei, `at` milliseconds. */
export type ManaTransfer = {
  id: string;
  hash: string;
  direction: "sent" | "received";
  counterparty: string;
  amount: string;
  at: number;
};

export type ManaTransfersPage = { transfers: ManaTransfer[]; next: ManaTransfersCursor | null };

export const MANA_TRANSFERS_PAGE_SIZE = 20;

type TransfersOptions = { cursor?: ManaTransfersCursor | null; signal?: AbortSignal };

/**
 * A page of `account`'s MANA transfers on `network`, newest first, from the node's transfer index. The
 * index holds confirmed transfers only, so a row is Sent or Received by its two addresses; it knows no
 * pending transfer and no bridge state. A transfer to oneself reads as sent.
 */
export async function fetchManaTransfers(
  account: string,
  network: WalletNetwork,
  opts: TransfersOptions = {},
): Promise<ManaTransfersPage> {
  if (!ADDRESS.test(account)) throw new RangeError("Not a wallet address");
  const self = account.toLowerCase();
  const raw = await getJSON<unknown>("/marketplace-api/v1/mana/transfers", {
    query: {
      network: WALLET_NETWORKS[network].index,
      address: self,
      limit: MANA_TRANSFERS_PAGE_SIZE,
      beforeBlock: opts.cursor?.beforeBlock,
      beforeLog: opts.cursor?.beforeLog,
    },
    signal: opts.signal,
  });
  const page = TransfersPageSchema.parse(raw);
  return {
    transfers: page.data.map((row) => {
      const sent = row.from.toLowerCase() === self;
      return {
        id: `${row.blockNumber}:${row.logIndex}`,
        hash: row.transactionHash,
        direction: sent ? "sent" : "received",
        counterparty: (sent ? row.to : row.from).toLowerCase(),
        amount: row.amount,
        at: row.timestamp * 1000,
      };
    }),
    next: page.next ?? null,
  };
}

/** The index answers 503 while the operator has it off or it has not reached the chain's head. */
export function isHistoryUnavailable(error: unknown): boolean {
  return error instanceof CatalystError && error.status === 503;
}

const WEI_PER_HUNDREDTH = 10n ** 16n;

/**
 * Wei as the section shows MANA: grouped, cut (never rounded up) to two decimals. An amount under a
 * hundredth is not zero, so it reads "< 0.01".
 */
export function formatManaAmount(wei: string): string {
  const value = BigInt(wei);
  if (value > 0n && value < WEI_PER_HUNDREDTH) return "< 0.01";
  return formatMana(manaWei(value), { group: true, decimals: 2 });
}
