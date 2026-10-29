// from: shop@94584a5c:app/src/lib/import.ts
// "Import your listings": bring a creator's classic (MANA-priced) listings of their own collections into
// the Shop as credit-buyable, USD-pegged primary listings. RESTRICTED to eligible primary inventory
// (SHOP.md; src/shop/policy.rs refuses `CreditListing` for anything else): a secondary token the seller
// owns is never re-listed for credits here. Its existing order stays what it is: a MANA listing, and a
// USD-pegged resale order keeps settling in MANA at a current quote. The refusal comes BEFORE the old
// listing is cancelled, so a refused import never leaves an item unlisted.

import { isShopApiError } from "../../api/client";
import { shopClient, type ShopClient } from "../../api/shop";
import type { ImportableListing } from "@catalyst-api/market/ImportableListing";
import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { CENTS_PER_CREDIT } from "../../model/money";
import {
  cancelTrade,
  createPrimaryUsdPeggedListing,
  ensureMinter,
  fetchTrade,
  hasApproval,
  isMarketplaceMinter,
  postTrade,
  TradeNotFoundError,
  type Trade,
  type TradeCreation,
} from "../../market/lib/trades";
import type { WaitOptions } from "../../market/lib/tx";

const SIX_MONTHS_MS = 1000 * 60 * 60 * 24 * 182;

export type ImportItem = Omit<ImportableListing, "listingType"> & {
  listingType: "primary" | "secondary";
  /** The server's credit quote for the old MANA price, as a starting point; null when it could not quote. */
  suggestedCredits: number | null;
};

/** Why an owned token has no credit import: it is a resale, MANA-only by the payment rule. */
export class ImportRestrictedError extends Error {
  override name = "ImportRestrictedError";
  constructor() {
    super("Only your own creations can be listed for credits. A token you own is a resale and keeps its MANA listing.");
  }
}

/**
 * Upstream `fetchImportable`: the seller's old listings split into creations (primary, importable for
 * credits) and owned tokens (secondary, shown as staying in MANA). Suggested prices come from the server's
 * credit quote of the old MANA amount, rounded up like any quote; a failed quote leaves them null.
 */
export async function fetchImportable(seller: string, c: ShopClient = shopClient()): Promise<{ creations: ImportItem[]; owned: ImportItem[] }> {
  const { data } = await c.catalog.importable({ seller: seller.toLowerCase() });
  const listings = data ?? [];
  if (listings.length === 0) return { creations: [], owned: [] };
  const primary = listings.filter((l) => l.listingType === "primary");
  let quotes: (string | null)[] = [];
  if (primary.length > 0) {
    try {
      quotes = (await c.credits.quotes({ amounts: primary.map((l) => l.manaWei) })).amounts;
    } catch {
      quotes = [];
    }
  }
  const creations = primary.map((l, i): ImportItem => {
    const credits = quotes[i] == null ? null : Number(quotes[i]);
    return { ...l, listingType: "primary", suggestedCredits: credits !== null && Number.isSafeInteger(credits) && credits > 0 ? credits : null };
  });
  const owned = listings.filter((l) => l.listingType !== "primary").map((l): ImportItem => ({ ...l, listingType: "secondary", suggestedCredits: null }));
  return { creations, owned };
}

/** What an import is doing right now, so the UI can say which wallet prompt or wait it is in. */
export type ImportPhase =
  | { step: "cancelling" }
  | { step: "confirming-cancel" }
  | { step: "authorising" }
  | { step: "signing" }
  | { step: "publishing" }
  | { step: "indexing"; attempt: number; of: number };

const CLEAR_RETRY_DELAYS_MS = [3000, 3000, 4000, 5000, 6000, 8000, 8000];

function stillOnSale(e: unknown): boolean {
  if (isShopApiError(e) && e.status === 409) return true;
  return /already an open order|status code 409/i.test((e as Error)?.message ?? "");
}

/**
 * Upstream `postListingWithRetry`: after the old listing is cancelled the indexer clears the item's "on sale"
 * flag a few seconds later, so the first POST can 409. Retry ONLY the POST (never re-sign) on that conflict.
 * `signal` ends the wait: an unmounted modal must not publish later.
 */
export async function postListingWithRetry(
  trade: TradeCreation,
  opts: { onPhase?: (phase: ImportPhase) => void; signal?: AbortSignal; post?: typeof postTrade; sleep?: (ms: number) => Promise<void> } = {},
): Promise<Trade> {
  const post = opts.post ?? postTrade;
  const sleep =
    opts.sleep ??
    ((ms: number) =>
      new Promise<void>((resolve, reject) => {
        const id = setTimeout(resolve, ms);
        opts.signal?.addEventListener(
          "abort",
          () => {
            clearTimeout(id);
            reject(opts.signal?.reason instanceof Error ? opts.signal.reason : new DOMException("Aborted", "AbortError"));
          },
          { once: true },
        );
      }));
  for (let attempt = 0; ; attempt++) {
    opts.signal?.throwIfAborted();
    try {
      opts.onPhase?.(attempt === 0 ? { step: "publishing" } : { step: "indexing", attempt, of: CLEAR_RETRY_DELAYS_MS.length });
      const created = await post(trade);
      opts.signal?.throwIfAborted();
      return created;
    } catch (e) {
      if (!stillOnSale(e) || attempt >= CLEAR_RETRY_DELAYS_MS.length) throw e;
      await sleep(CLEAR_RETRY_DELAYS_MS[attempt]!);
    }
  }
}

/** The old listing was taken down but the new one did not post: the item is now unlisted. */
export class RelistFailedError extends Error {
  override name = "RelistFailedError";
  constructor(readonly original: unknown) {
    super("Your old listing was removed, but listing it in the Shop failed. List it again from My Store.");
  }
}

/**
 * Upstream `countConfirmations`: two wallet prompts per item (the cancel, the signature) plus one per
 * collection whose mint right is not granted yet. Null when a read fails: a wrong number is worse than none.
 */
export async function countConfirmations(
  items: readonly ImportItem[],
  owner: string,
  reads: { isMinter?: typeof isMarketplaceMinter; approved?: typeof hasApproval } = {},
): Promise<{ perItem: number; approvals: number; total: number } | null> {
  const PER_ITEM = 2;
  try {
    const byCollection = new Map<string, ImportItem>();
    for (const item of items) {
      const key = `${item.contractAddress.toLowerCase()}:${item.listingType}:${item.chainId}`;
      if (!byCollection.has(key)) byCollection.set(key, item);
    }
    const granted = await Promise.all(
      [...byCollection.values()].map((item) =>
        item.listingType === "primary"
          ? (reads.isMinter ?? isMarketplaceMinter)({ contractAddress: item.contractAddress, chainId: item.chainId as ChainId })
          : (reads.approved ?? hasApproval)({ contractAddress: item.contractAddress, chainId: item.chainId as ChainId, owner }),
      ),
    );
    const approvals = granted.filter((ok) => !ok).length;
    return { perItem: PER_ITEM, approvals, total: items.length * PER_ITEM + approvals };
  } catch {
    return null;
  }
}

/**
 * Upstream `importListing`, primary only: take the old MANA listing down (confirmed on chain), ensure the
 * marketplace may mint the collection, sign a USD-pegged COLLECTION_ITEM order at `priceCredits` (one credit
 * is ten US cents) for the remaining supply, and post it. `cancelOld: false` skips a cancel already done.
 */
export async function importListing(
  item: ImportItem,
  priceCredits: number,
  opts: {
    provider: Eip1193Provider;
    cancelOld?: boolean;
    onPhase?: (phase: ImportPhase) => void;
    now?: () => number;
    wait?: WaitOptions;
    sleep?: (ms: number) => Promise<void>;
  },
): Promise<Trade> {
  if (item.listingType !== "primary" || !item.itemId) throw new ImportRestrictedError();
  if (!Number.isSafeInteger(priceCredits) || priceCredits < 1) throw new RangeError("A price is a whole number of credits, at least 1");
  const chainId = item.chainId as ChainId;
  let removedOld = false;
  if (opts.cancelOld !== false) {
    const old = await fetchTrade(item.oldTradeId).catch((e: unknown) => {
      if (e instanceof TradeNotFoundError) return null;
      throw e;
    });
    if (old) {
      opts.onPhase?.({ step: "cancelling" });
      const cancelled = cancelTrade({ provider: opts.provider, trade: old, wait: opts.wait });
      opts.onPhase?.({ step: "confirming-cancel" });
      await cancelled;
      removedOld = true;
    }
  }
  try {
    opts.onPhase?.({ step: "authorising" });
    await ensureMinter({ provider: opts.provider, contractAddress: item.contractAddress, chainId, wait: opts.wait });
    opts.onPhase?.({ step: "signing" });
    const now = (opts.now ?? Date.now)();
    const trade = await createPrimaryUsdPeggedListing(
      {
        provider: opts.provider,
        item: { contractAddress: item.contractAddress, itemId: item.itemId, chainId },
        usdCents: priceCredits * CENTS_PER_CREDIT,
        uses: item.available,
        expiresAtMs: now + SIX_MONTHS_MS,
      },
      { now: () => now },
    );
    return await postListingWithRetry(trade, { onPhase: opts.onPhase, sleep: opts.sleep });
  } catch (e) {
    if (removedOld) throw new RelistFailedError(e);
    throw e;
  }
}
