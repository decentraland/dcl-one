import { describe, expect, it } from 'vitest';

import {
  ANALYTICS_WATCHLIST_VERSION,
  watchlistFor,
  withWatchlist,
} from '../types/analytics-watchlist';

const WALLET = '0x00000000000000000000000000000000000000AA';
const WALLET_KEY = WALLET.toLowerCase();
const OTHER_WALLET = '0x0000000000000000000000000000000000000001';
const PLACE = 'world:example.dcl.eth@0,0';
const OTHER_PLACE = 'land:10,20';

const entry = (placeIds: unknown, version: unknown = ANALYTICS_WATCHLIST_VERSION) => ({
  version,
  placeIds,
});

describe('watchlistFor', () => {
  describe('when the wallet has a stored watchlist', () => {
    it('should return its place ids, whatever the case of the address', () => {
      const stored = { [WALLET_KEY]: entry([PLACE, OTHER_PLACE]) };

      expect(watchlistFor(stored, WALLET)).toEqual([PLACE, OTHER_PLACE]);
      expect(watchlistFor(stored, WALLET_KEY)).toEqual([PLACE, OTHER_PLACE]);
    });

    it('should drop entries that are not place ids, and repeats', () => {
      const stored = { [WALLET_KEY]: entry([PLACE, 7, null, PLACE, OTHER_PLACE]) };

      expect(watchlistFor(stored, WALLET)).toEqual([PLACE, OTHER_PLACE]);
    });
  });

  describe('when nothing is stored for the wallet', () => {
    it('should return an empty list', () => {
      expect(watchlistFor(undefined, WALLET)).toEqual([]);
      expect(watchlistFor({}, WALLET)).toEqual([]);
      expect(watchlistFor({ [OTHER_WALLET]: entry([PLACE]) }, WALLET)).toEqual([]);
    });
  });

  describe('when there is no address', () => {
    it('should return an empty list', () => {
      expect(watchlistFor({ [WALLET_KEY]: entry([PLACE]) }, null)).toEqual([]);
      expect(watchlistFor({ [WALLET_KEY]: entry([PLACE]) }, '')).toEqual([]);
    });
  });

  describe('when the stored value is malformed', () => {
    it.each([
      ['a string map', 'not a map'],
      ['an array map', [entry([PLACE])]],
      ['a string entry', { [WALLET_KEY]: 'not an entry' }],
      ['an array entry', { [WALLET_KEY]: [PLACE] }],
      ['ids that are not a list', { [WALLET_KEY]: entry(PLACE) }],
      ['no version', { [WALLET_KEY]: { placeIds: [PLACE] } }],
    ])('should return an empty list for %s', (_, stored) => {
      expect(watchlistFor(stored, WALLET)).toEqual([]);
    });

    it('should not read a key inherited from the prototype', () => {
      expect(watchlistFor({}, '__proto__')).toEqual([]);
    });
  });

  describe('when the stored watchlist has another version', () => {
    it('should return an empty list', () => {
      const stored = { [WALLET_KEY]: entry([PLACE], ANALYTICS_WATCHLIST_VERSION - 1) };

      expect(watchlistFor(stored, WALLET)).toEqual([]);
    });
  });
});

describe('withWatchlist', () => {
  describe('when the wallet pins places', () => {
    it('should store them under the lowercased address with the current version', () => {
      expect(withWatchlist(undefined, WALLET, [PLACE])).toEqual({
        [WALLET_KEY]: { version: ANALYTICS_WATCHLIST_VERSION, placeIds: [PLACE] },
      });
    });

    it('should keep every other wallet as stored', () => {
      const other = entry([OTHER_PLACE]);

      expect(withWatchlist({ [OTHER_WALLET]: other }, WALLET, [PLACE])).toEqual({
        [OTHER_WALLET]: other,
        [WALLET_KEY]: { version: ANALYTICS_WATCHLIST_VERSION, placeIds: [PLACE] },
      });
    });

    it('should replace the wallet list rather than join it', () => {
      const stored = { [WALLET_KEY]: entry([PLACE]) };

      expect(watchlistFor(withWatchlist(stored, WALLET, [OTHER_PLACE]), WALLET)).toEqual([
        OTHER_PLACE,
      ]);
    });

    it('should store each place once', () => {
      expect(watchlistFor(withWatchlist(undefined, WALLET, [PLACE, PLACE]), WALLET)).toEqual([
        PLACE,
      ]);
    });
  });

  describe('when the wallet unpins its last place', () => {
    it('should remove its entry', () => {
      const stored = { [WALLET_KEY]: entry([PLACE]), [OTHER_WALLET]: entry([PLACE]) };

      expect(withWatchlist(stored, WALLET, [])).toEqual({ [OTHER_WALLET]: entry([PLACE]) });
    });
  });

  describe('when the stored value is malformed', () => {
    it('should replace it as a whole', () => {
      expect(withWatchlist('not a map', WALLET, [PLACE])).toEqual({
        [WALLET_KEY]: { version: ANALYTICS_WATCHLIST_VERSION, placeIds: [PLACE] },
      });
    });

    it('should drop entries of other wallets that are not objects', () => {
      expect(withWatchlist({ [OTHER_WALLET]: 'broken' }, WALLET, [PLACE])).toEqual({
        [WALLET_KEY]: { version: ANALYTICS_WATCHLIST_VERSION, placeIds: [PLACE] },
      });
    });
  });
});
