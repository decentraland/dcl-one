/**
 * Bump when what a place id means changes (its format, or the scenes it names): every stored
 * watchlist is then read as empty instead of pinning ids that no longer match any place.
 */
export const ANALYTICS_WATCHLIST_VERSION = 1;

/** One wallet's Analytics watchlist: the places pinned to the top of the list. */
export interface AnalyticsWatchlist {
  version: number;
  placeIds: string[];
}

/**
 * `AppSettings.analyticsWatchlist`: watchlists per wallet (lowercase address). Pins belong to
 * the account, so a second wallet signing in on the same machine starts with none.
 */
export type AnalyticsWatchlistByWallet = Record<string, AnalyticsWatchlist>;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function uniqueStrings(values: unknown[]): string[] {
  return [...new Set(values.filter((value): value is string => typeof value === 'string'))];
}

/**
 * The places `address` pinned. A missing, malformed or other-version value reads as no pins,
 * never as an error: the stored value is the creator's own file and may be anything.
 */
export function watchlistFor(byWallet: unknown, address: string | null | undefined): string[] {
  if (!address || !isPlainObject(byWallet)) return [];
  const key = address.toLowerCase();
  if (!Object.hasOwn(byWallet, key)) return [];
  const entry = byWallet[key];
  if (!isPlainObject(entry) || entry.version !== ANALYTICS_WATCHLIST_VERSION) return [];
  return Array.isArray(entry.placeIds) ? uniqueStrings(entry.placeIds) : [];
}

/**
 * The stored watchlists with `address`'s replaced by `placeIds`. Other wallets keep theirs; an
 * empty list removes the wallet's entry. A malformed stored value is replaced as a whole.
 */
export function withWatchlist(
  byWallet: unknown,
  address: string,
  placeIds: string[],
): AnalyticsWatchlistByWallet {
  const key = address.toLowerCase();
  const others: AnalyticsWatchlistByWallet = {};
  if (isPlainObject(byWallet)) {
    // Kept as stored, valid or not: `watchlistFor` checks each entry when it is read.
    for (const [wallet, entry] of Object.entries(byWallet)) {
      if (wallet !== key && isPlainObject(entry)) {
        others[wallet] = entry as unknown as AnalyticsWatchlist;
      }
    }
  }
  const pinned = uniqueStrings(placeIds);
  if (pinned.length === 0) return others;
  return { ...others, [key]: { version: ANALYTICS_WATCHLIST_VERSION, placeIds: pinned } };
}
