import { beforeEach, describe, expect, it, vi } from 'vitest';

import { settings as settingsPreload } from '#preload';

import { MetricsWindow, SortBy } from '../../../../../shared/types/place-analytics';
import { watchlistFor, withWatchlist } from '../../../../../shared/types/analytics-watchlist';
import { FilterBy } from '../../../../../shared/types/manage';
import { AuthServerProvider } from '../../../lib/auth';
import type { AnalyticsPlace } from '../../../lib/analyticsLocations';
import type { LocationMetrics } from '../../../lib/metricsApi';
import { fetchAnalytics as fetchAnalyticsSnapshot } from '../../../lib/placeAnalytics';
import { createTestStore } from '../../../../tests/utils/testStore';
import type * as ManagementUtils from '../management/utils';
import { fetchAllDeployedWorlds } from '../management/utils';
import { actions as managementActions } from '../management';
import { accountChanged } from '../account';
import { actions, fetchAnalytics, initialState, selectors } from './slice';

const TEST_ADDRESS = '0x123abc';
const EXPORTED_AT = '2026-08-12T00:17:01.099Z';

const place = (placeId: string, name: string): AnalyticsPlace => ({
  placeId,
  name,
  thumbnail: 'thumb.png',
  location: { world: name, x: 0, y: 0 },
  publishedIn: name,
  lastUpdatedAt: null,
});

const withVisits = (visits: number): LocationMetrics =>
  ({
    location_key: 'unused',
    x: 0,
    y: 0,
    builder_project_id: null,
    metrics: { unique_visits_60d: [{ series: 'all', period: null, value: visits }] },
  }) as LocationMetrics;

const EMPTY_METRICS: LocationMetrics = {
  location_key: 'unused',
  x: 0,
  y: 0,
  builder_project_id: null,
  metrics: {},
} as LocationMetrics;

const BANANARAMA = place('world:bananarama.dcl.eth@0,0', 'bananarama.dcl.eth');
const NIGHTMARE = place('world:nightmare.dcl.eth@0,0', 'nightmare.dcl.eth');

const SNAPSHOT = {
  exportedAt: EXPORTED_AT,
  places: [BANANARAMA, NIGHTMARE],
  metricsByPlaceId: {
    [BANANARAMA.placeId]: withVisits(2000),
    [NIGHTMARE.placeId]: withVisits(500),
  },
};

vi.mock('../../../lib/placeAnalytics', () => ({ fetchAnalytics: vi.fn() }));

vi.mock('../../../lib/auth', () => ({
  AuthServerProvider: { getAccount: vi.fn() },
}));

vi.mock('../../../lib/worlds', async () => {
  const actual = await import('../../../lib/worlds');
  return { ...actual, Worlds: vi.fn() };
});

vi.mock('../management/utils', async importOriginal => ({
  ...(await importOriginal<ManagementUtils>()),
  fetchAllDeployedWorlds: vi.fn(),
}));

vi.mock('/@/modules/store/land', async () => {
  const actual = await import('../land');

  /** What `dispatch(thunk())` resolves to: a promise that also carries `unwrap`. */
  const asDispatchedThunk = <T>(value: T) =>
    Object.assign(Promise.resolve(value), { unwrap: () => Promise.resolve(value) });

  return {
    ...actual,
    fetchLandList: vi.fn(() => () => asDispatchedThunk({ land: [] as unknown[] })),
  };
});

describe('placeAnalytics slice', () => {
  let store: ReturnType<typeof createTestStore>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(fetchAllDeployedWorlds).mockResolvedValue([]);
    store = createTestStore();
  });

  describe('when the manage page has already narrowed its own list', () => {
    const OWN_WORLDS = [{ id: 'bananarama.dcl.eth' }] as any;

    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      vi.mocked(fetchAllDeployedWorlds).mockResolvedValue(OWN_WORLDS);
    });

    it('should build the list from its own worlds fetch, not the manage page state', async () => {
      store.dispatch(managementActions.setSearchQuery('nothing-matches-this'));
      store.dispatch(managementActions.setPublishFilter(FilterBy.UNPUBLISHED));

      await store.dispatch(fetchAnalytics());

      expect(fetchAllDeployedWorlds).toHaveBeenCalledWith(expect.anything(), TEST_ADDRESS);
      expect(fetchAnalyticsSnapshot).toHaveBeenCalledWith(OWN_WORLDS, []);
    });

    it('should not read a management fetch that is still in flight', async () => {
      store.dispatch(managementActions.setPage(3));

      await store.dispatch(fetchAnalytics());

      expect(fetchAnalyticsSnapshot).toHaveBeenCalledWith(OWN_WORLDS, []);
    });
  });

  describe('when the account changes while a fetch is in flight', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      store.dispatch(actions.togglePinnedPlace(BANANARAMA.placeId));
      const late = store.dispatch(fetchAnalytics());
      store.dispatch(accountChanged());
      await late;
    });

    it("should drop the previous account's places, pins and late answer", () => {
      const state = store.getState().placeAnalytics;
      expect(state.places).toEqual([]);
      expect(state.metricsByPlaceId).toEqual({});
      expect(state.pinnedPlaceIds).toEqual([]);
      expect(state.status).toBe('idle');
    });
  });

  describe('when a fetch is already in flight', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAllDeployedWorlds).mockResolvedValue([]);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
    });

    it('should not start a second one that could overwrite it with staler data', async () => {
      await Promise.all([store.dispatch(fetchAnalytics()), store.dispatch(fetchAnalytics())]);

      expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1);
    });
  });

  describe('when fetching analytics with a connected account', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAllDeployedWorlds).mockResolvedValue([]);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
    });

    it('should store the snapshot and mark the request as succeeded', async () => {
      await store.dispatch(fetchAnalytics());

      expect(store.getState().placeAnalytics).toEqual({
        ...initialState,
        exportedAt: EXPORTED_AT,
        places: SNAPSHOT.places,
        metricsByPlaceId: SNAPSHOT.metricsByPlaceId,
        requestId: expect.any(String),
        status: 'succeeded',
      });
    });

    it('should ask for every scene in one call rather than one call per tab', async () => {
      await store.dispatch(fetchAnalytics());

      expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the creator has no scenes', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue({
        exportedAt: '',
        places: [],
        metricsByPlaceId: {},
      });
    });

    it('should succeed with an empty list rather than failing', async () => {
      await store.dispatch(fetchAnalytics());

      const { places, status } = store.getState().placeAnalytics;
      expect(places).toEqual([]);
      expect(status).toBe('succeeded');
    });
  });

  describe('when fetching analytics without a connected account', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(undefined as any);
    });

    it('should fail without requesting anything', async () => {
      await store.dispatch(fetchAnalytics());

      expect(fetchAnalyticsSnapshot).not.toHaveBeenCalled();
      expect(store.getState().placeAnalytics.status).toBe('failed');
    });
  });

  describe('when the request fails', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockRejectedValue(
        new Error('locations[3]: "Not A Name" is not a valid ENS name'),
      );
    });

    it('should keep the message the service gave, so the page can show it', async () => {
      await store.dispatch(fetchAnalytics());

      const { status, error } = store.getState().placeAnalytics;
      expect(status).toBe('failed');
      expect(error).toMatch(/is not a valid ENS name/);
    });
  });

  describe('when loading the project list fails', () => {
    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAllDeployedWorlds).mockRejectedValue(new Error('could not load your scenes'));
    });

    it('should fail rather than report an empty list of places', async () => {
      await store.dispatch(fetchAnalytics());

      const { status, places, error } = store.getState().placeAnalytics;
      expect(status).toBe('failed');
      expect(places).toEqual([]);
      expect(error).toMatch(/could not load your scenes/);
      expect(fetchAnalyticsSnapshot).not.toHaveBeenCalled();
    });
  });

  describe('when the account is disconnected', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      await store.dispatch(fetchAnalytics());
    });

    it('should drop the previous account analytics', async () => {
      expect(store.getState().placeAnalytics.places).not.toEqual([]);

      store.dispatch(accountChanged());

      expect(store.getState().placeAnalytics).toEqual(initialState);
    });
  });

  describe('when the account is disconnected while a fetch is in flight', () => {
    let resolveSnapshot: (snapshot: typeof SNAPSHOT) => void;

    beforeEach(() => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockReturnValue(
        new Promise(resolve => {
          resolveSnapshot = resolve;
        }),
      );
    });

    it('should not let the previous account analytics land once it settles', async () => {
      const inFlight = store.dispatch(fetchAnalytics());
      await vi.waitFor(() => expect(fetchAnalyticsSnapshot).toHaveBeenCalled());

      store.dispatch(accountChanged());
      resolveSnapshot(SNAPSHOT);
      await inFlight;

      expect(store.getState().placeAnalytics).toEqual(initialState);
    });
  });

  describe('when reading the visible places', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      await store.dispatch(fetchAnalytics());
    });

    it('should project each scene into a summary row', () => {
      const [first] = selectors.getVisiblePlaces(store.getState());

      expect(first).toMatchObject({ placeId: BANANARAMA.placeId, name: BANANARAMA.name });
    });

    it('should apply the selected sorting', () => {
      store.dispatch(actions.setSortBy(SortBy.NAME_DESC));

      expect(selectors.getVisiblePlaces(store.getState()).map(p => p.name)).toEqual([
        NIGHTMARE.name,
        BANANARAMA.name,
      ]);
    });

    it('should sort on visits, which come from the visits metric', () => {
      store.dispatch(actions.setSortBy(SortBy.MOST_VISITS));

      expect(selectors.getVisiblePlaces(store.getState()).map(p => p.totalVisits)).toEqual([
        2000, 500,
      ]);
    });

    it('should apply the search query', () => {
      store.dispatch(actions.setSearchQuery('night'));

      expect(selectors.getVisiblePlaces(store.getState()).map(p => p.name)).toEqual([
        NIGHTMARE.name,
      ]);
    });

    it('should list pinned places first', () => {
      store.dispatch(actions.togglePinnedPlace(NIGHTMARE.placeId));

      expect(selectors.getVisiblePlaces(store.getState())[0].name).toBe(NIGHTMARE.name);
    });
  });

  describe('when the window changes', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      await store.dispatch(fetchAnalytics());
    });

    it('should re-read the same snapshot rather than fetching again', () => {
      store.dispatch(actions.setWindow(MetricsWindow.LAST_30_DAYS));

      expect(store.getState().placeAnalytics.window).toBe(MetricsWindow.LAST_30_DAYS);
      expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1);
    });

    it('should leave a scalar empty when the other window is the one that has it', () => {
      store.dispatch(actions.setWindow(MetricsWindow.LAST_30_DAYS));

      // The fixture only carries the 60-day metric, so the 30-day read is null.
      expect(selectors.getVisiblePlaces(store.getState())[0].totalVisits).toBeNull();
    });
  });

  describe('when a scene has no metrics at all', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue({
        exportedAt: EXPORTED_AT,
        places: [BANANARAMA],
        metricsByPlaceId: { [BANANARAMA.placeId]: EMPTY_METRICS },
      });
      await store.dispatch(fetchAnalytics());
    });

    it('should keep it in the list, flagged rather than dropped or failed', () => {
      const [row] = selectors.getVisiblePlaces(store.getState());

      expect(row.hasNoData).toBe(true);
      expect(store.getState().placeAnalytics.status).toBe('succeeded');
    });

    it('should still surface the export stamp', () => {
      expect(store.getState().placeAnalytics.exportedAt).toBe(EXPORTED_AT);
    });
  });

  describe('when reading one scene', () => {
    beforeEach(async () => {
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(TEST_ADDRESS);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
      await store.dispatch(fetchAnalytics());
    });

    it('should find it by our own id', () => {
      expect(selectors.getPlace(store.getState(), BANANARAMA.placeId)).toEqual(BANANARAMA);
      expect(selectors.getPlaceMetrics(store.getState(), BANANARAMA.placeId)).toEqual(
        SNAPSHOT.metricsByPlaceId[BANANARAMA.placeId],
      );
    });

    it('should return nothing for a scene that is not in the snapshot', () => {
      expect(selectors.getPlace(store.getState(), 'land:99,99')).toBeUndefined();
      expect(selectors.getPlaceMetrics(store.getState(), 'land:99,99')).toBeUndefined();
    });
  });
});

describe('placeAnalytics watchlist', () => {
  const WALLET = '0x00000000000000000000000000000000000000AA';
  const WALLET_KEY = WALLET.toLowerCase();
  const OTHER_WALLET = '0x0000000000000000000000000000000000000001';

  /** The stored `AppSettings.analyticsWatchlist`, read and written as the preload bridge does. */
  let stored: unknown;
  let store: ReturnType<typeof createTestStore>;

  const signInAs = (address: string) =>
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue(address);

  const load = (address: string) => store.dispatch(actions.loadPinnedPlaces({ address }));

  const visibleNames = () => selectors.getVisiblePlaces(store.getState()).map(p => p.name);

  beforeEach(() => {
    vi.clearAllMocks();
    stored = undefined;
    vi.mocked(settingsPreload.getAnalyticsWatchlist).mockImplementation(async address =>
      watchlistFor(stored, address),
    );
    vi.mocked(settingsPreload.setAnalyticsWatchlist).mockImplementation(
      async (address, placeIds) => {
        stored = withWatchlist(stored, address, placeIds);
      },
    );
    vi.mocked(fetchAllDeployedWorlds).mockResolvedValue([]);
    vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue(SNAPSHOT);
    signInAs(WALLET);
    store = createTestStore();
  });

  describe('when a place is toggled after the stored watchlist loaded', () => {
    beforeEach(async () => {
      await load(WALLET_KEY);
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should pin it in the store', () => {
      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([NIGHTMARE.placeId]);
    });

    it('should save the list for the lowercased wallet', () => {
      expect(settingsPreload.setAnalyticsWatchlist).toHaveBeenCalledWith(WALLET_KEY, [
        NIGHTMARE.placeId,
      ]);
      expect(watchlistFor(stored, WALLET)).toEqual([NIGHTMARE.placeId]);
    });

    it('should save the removal when it is toggled again', async () => {
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));

      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([]);
      expect(watchlistFor(stored, WALLET)).toEqual([]);
    });
  });

  describe('when a place is toggled before the stored watchlist loaded', () => {
    beforeEach(async () => {
      stored = withWatchlist(undefined, WALLET, [BANANARAMA.placeId]);
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should not replace the stored list', () => {
      expect(settingsPreload.setAnalyticsWatchlist).not.toHaveBeenCalled();
      expect(watchlistFor(stored, WALLET)).toEqual([BANANARAMA.placeId]);
    });
  });

  describe('when the stored watchlist is loaded in a new session', () => {
    beforeEach(async () => {
      await load(WALLET_KEY);
      await store.dispatch(actions.togglePin(BANANARAMA.placeId));

      store = createTestStore();
      await load(WALLET);
      await store.dispatch(fetchAnalytics());
    });

    it('should restore the pins', () => {
      const { pinnedPlaceIds, watchlist } = store.getState().placeAnalytics;
      expect(pinnedPlaceIds).toEqual([BANANARAMA.placeId]);
      expect(watchlist).toMatchObject({ address: WALLET_KEY, status: 'succeeded' });
    });

    it('should list the pinned places first', () => {
      store.dispatch(actions.setSortBy(SortBy.NAME_DESC));

      expect(visibleNames()).toEqual([BANANARAMA.name, NIGHTMARE.name]);
    });
  });

  describe('when the stored value is missing or malformed', () => {
    it('should load an empty list', async () => {
      stored = { [WALLET_KEY]: 'not a watchlist' };

      await load(WALLET_KEY);

      const { pinnedPlaceIds, watchlist } = store.getState().placeAnalytics;
      expect(pinnedPlaceIds).toEqual([]);
      expect(watchlist.status).toBe('succeeded');
    });
  });

  describe('when reading the stored watchlist fails', () => {
    let toggled: { type: string };

    beforeEach(async () => {
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockRejectedValue(new Error('no config'));
      await load(WALLET_KEY);
      toggled = await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should still pin in the store without saving over what may be stored', () => {
      expect(store.getState().placeAnalytics.watchlist.status).toBe('failed');
      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([NIGHTMARE.placeId]);
      expect(settingsPreload.setAnalyticsWatchlist).not.toHaveBeenCalled();
    });

    it('should read the stored list again before the toggle', () => {
      expect(settingsPreload.getAnalyticsWatchlist).toHaveBeenCalledTimes(2);
    });

    it('should report that the pin was not saved', () => {
      expect(toggled.type).toBe(actions.togglePin.rejected.type);
    });
  });

  describe('when reading the stored watchlist fails once and a place is then toggled', () => {
    beforeEach(async () => {
      stored = withWatchlist(undefined, WALLET, [BANANARAMA.placeId]);
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockRejectedValueOnce(
        new Error('no config'),
      );
      await load(WALLET_KEY);
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should add the pin to the stored list and save it', () => {
      expect(watchlistFor(stored, WALLET)).toEqual([BANANARAMA.placeId, NIGHTMARE.placeId]);
    });
  });

  describe('when reading the stored watchlist fails once and a stored place is then pinned', () => {
    beforeEach(async () => {
      stored = withWatchlist(undefined, WALLET, [NIGHTMARE.placeId]);
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockRejectedValueOnce(
        new Error('no config'),
      );
      await load(WALLET_KEY);
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should keep the place pinned, as the click asked, instead of unpinning it', () => {
      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([NIGHTMARE.placeId]);
      expect(watchlistFor(stored, WALLET)).toEqual([NIGHTMARE.placeId]);
    });
  });

  describe('when saving the watchlist fails', () => {
    let toggled: { type: string };

    beforeEach(async () => {
      await load(WALLET_KEY);
      vi.mocked(settingsPreload.setAnalyticsWatchlist).mockRejectedValue(new Error('disk full'));
      toggled = await store.dispatch(actions.togglePin(NIGHTMARE.placeId));
    });

    it('should report that the pin was not saved', () => {
      expect(toggled.type).toBe(actions.togglePin.rejected.type);
    });
  });

  describe('when a pinned place is not in the fetched list', () => {
    const GONE = 'world:gone.dcl.eth@0,0';

    beforeEach(async () => {
      stored = withWatchlist(undefined, WALLET, [GONE]);
      await load(WALLET_KEY);
      await store.dispatch(fetchAnalytics());
    });

    it('should leave it out of the list', () => {
      expect(visibleNames()).toEqual([BANANARAMA.name, NIGHTMARE.name]);
    });

    it('should keep it stored when another place is pinned', async () => {
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));

      expect(watchlistFor(stored, WALLET)).toEqual([GONE, NIGHTMARE.placeId]);
    });
  });

  describe('when the fetch fails or comes back empty', () => {
    beforeEach(async () => {
      stored = withWatchlist(undefined, WALLET, [BANANARAMA.placeId]);
      await load(WALLET_KEY);
    });

    it('should keep the stored pins after a failed fetch', async () => {
      vi.mocked(fetchAnalyticsSnapshot).mockRejectedValue(new Error('offline'));

      await store.dispatch(fetchAnalytics());

      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([BANANARAMA.placeId]);
      expect(watchlistFor(stored, WALLET)).toEqual([BANANARAMA.placeId]);
    });

    it('should keep the stored pins after an empty fetch', async () => {
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue({
        exportedAt: '',
        places: [],
        metricsByPlaceId: {},
      });

      await store.dispatch(fetchAnalytics());

      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([BANANARAMA.placeId]);
      expect(watchlistFor(stored, WALLET)).toEqual([BANANARAMA.placeId]);
    });
  });

  describe('when the account switches', () => {
    beforeEach(async () => {
      await load(WALLET_KEY);
      await store.dispatch(actions.togglePin(BANANARAMA.placeId));

      store.dispatch(accountChanged());
      signInAs(OTHER_WALLET);
      await load(OTHER_WALLET);
    });

    it("should not show the previous account's pins", () => {
      const { pinnedPlaceIds, watchlist } = store.getState().placeAnalytics;
      expect(pinnedPlaceIds).toEqual([]);
      expect(watchlist.address).toBe(OTHER_WALLET);
    });

    it("should save the new account's pins apart from the previous account's", async () => {
      await store.dispatch(actions.togglePin(NIGHTMARE.placeId));

      expect(watchlistFor(stored, OTHER_WALLET)).toEqual([NIGHTMARE.placeId]);
      expect(watchlistFor(stored, WALLET)).toEqual([BANANARAMA.placeId]);
    });

    it('should bring back the first account pins when it signs in again', async () => {
      store.dispatch(accountChanged());
      signInAs(WALLET);
      await load(WALLET_KEY);

      expect(store.getState().placeAnalytics.pinnedPlaceIds).toEqual([BANANARAMA.placeId]);
    });
  });

  describe('when the account switches while the previous one is loading', () => {
    it("should drop the previous account's late answer", async () => {
      stored = withWatchlist(undefined, WALLET, [BANANARAMA.placeId]);
      let release: () => void = () => {};
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockImplementationOnce(
        address =>
          new Promise(resolve => {
            release = () => resolve(watchlistFor(stored, address));
          }),
      );
      const late = load(WALLET_KEY);

      store.dispatch(accountChanged());
      signInAs(OTHER_WALLET);
      await load(OTHER_WALLET);
      release();
      await late;

      const { pinnedPlaceIds, watchlist } = store.getState().placeAnalytics;
      expect(pinnedPlaceIds).toEqual([]);
      expect(watchlist).toMatchObject({ address: OTHER_WALLET, status: 'succeeded' });
    });
  });

  describe('when asked to load a wallet that is not the one signed in', () => {
    it('should not read it', async () => {
      signInAs(OTHER_WALLET);

      await load(WALLET_KEY);

      expect(settingsPreload.getAnalyticsWatchlist).not.toHaveBeenCalled();
      expect(store.getState().placeAnalytics.watchlist.status).toBe('idle');
    });
  });
});
