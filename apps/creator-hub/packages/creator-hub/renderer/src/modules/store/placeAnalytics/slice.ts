import { createSelector, createSlice, type PayloadAction } from '@reduxjs/toolkit';
import { settings as settingsPreload } from '#preload';

import type { Async, Status } from '/shared/types/async';
import { MetricsWindow, SortBy } from '/shared/types/place-analytics';

import { AuthServerProvider } from '/@/lib/auth';
import type { AnalyticsPlace } from '/@/lib/analyticsLocations';
import type { LocationMetrics } from '/@/lib/metricsApi';
import { toSummary } from '/@/lib/placeAnalytics.adapter';
import { fetchAnalytics as fetchAnalyticsSnapshot } from '/@/lib/placeAnalytics';
import { Worlds } from '/@/lib/worlds';
import type { AppState } from '/@/modules/store';
import { accountChanged, isSignedInAccount } from '/@/modules/store/account';
import { fetchLandList } from '/@/modules/store/land';
import { fetchAllDeployedWorlds } from '/@/modules/store/management/utils';
import { createAsyncThunk } from '/@/modules/store/thunk';

import { sortPlaces } from './utils';

/**
 * Analytics for every scene the connected account owns or collaborates on.
 *
 * One batched request answers the whole feature — the list and every tab of the
 * detail page — so this is the only thunk. Switching window or opening a scene
 * re-reads the same snapshot rather than fetching again.
 *
 * The scenes to ask about are fetched here rather than read out of the
 * management slice: that slice holds the Manage page's own list, narrowed by its
 * search box, filter and page, so borrowing it makes this list silently inherit
 * a view the creator set somewhere else.
 *
 * Skipped while one is already running: a second run would rebuild the list from
 * whatever the store held when it started, and the slower one wins.
 */
export const fetchAnalytics = createAsyncThunk(
  'placeAnalytics/fetchAnalytics',
  async (_: void, { dispatch }) => {
    const connectedAccount = AuthServerProvider.getAccount();
    if (!connectedAccount) throw new Error('No connected account found');

    const [projects, { land }] = await Promise.all([
      fetchAllDeployedWorlds(new Worlds(), connectedAccount),
      dispatch(fetchLandList({ address: connectedAccount })).unwrap(),
    ]);

    return fetchAnalyticsSnapshot(projects, land);
  },
  {
    condition: (_: void, { getState }) =>
      (getState() as AppState).placeAnalytics.status !== 'loading',
  },
);

/**
 * Reads the account's watchlist from the app settings (`AppSettings.analyticsWatchlist`).
 * Skipped unless `address` is the signed-in account, so a wallet that is being replaced never
 * loads its pins into the next one's list.
 */
export const loadPinnedPlaces = createAsyncThunk(
  'placeAnalytics/loadPinnedPlaces',
  async ({ address }: { address: string }) => settingsPreload.getAnalyticsWatchlist(address),
  { condition: isSignedInAccount },
);

export const savePinnedPlaces = createAsyncThunk(
  'placeAnalytics/savePinnedPlaces',
  async ({ address, placeIds }: { address: string; placeIds: string[] }) =>
    settingsPreload.setAnalyticsWatchlist(address, placeIds),
  { condition: isSignedInAccount },
);

// state
export type PlaceAnalyticsState = {
  /** The warehouse's export stamp, shown as an "as of" date. */
  exportedAt: string;
  places: AnalyticsPlace[];
  metricsByPlaceId: Record<string, LocationMetrics>;
  /** Which trailing window the scalar metrics are read over. */
  window: MetricsWindow;
  sortBy: SortBy;
  searchQuery: string;
  /**
   * Places pinned to the watchlist, shown first in the list. Saved per wallet in the app
   * settings; may name places the current snapshot lacks, which the list simply leaves out.
   */
  pinnedPlaceIds: string[];
  /** Loading of the stored watchlist: for which wallet (lowercase), and how far it got. */
  watchlist: { address: string | null; status: Status; requestId: string | null };
  requestId: string | null;
};

export const initialState: Async<PlaceAnalyticsState> = {
  exportedAt: '',
  places: [],
  metricsByPlaceId: {},
  window: MetricsWindow.LAST_60_DAYS,
  sortBy: SortBy.NAME_ASC,
  searchQuery: '',
  pinnedPlaceIds: [],
  watchlist: { address: null, status: 'idle', requestId: null },
  requestId: null,
  status: 'idle',
  error: null,
};

// slice
const slice = createSlice({
  name: 'placeAnalytics',
  initialState,
  reducers: {
    setSortBy: (state, action: PayloadAction<SortBy>) => {
      state.sortBy = action.payload;
    },
    setSearchQuery: (state, action: PayloadAction<string>) => {
      state.searchQuery = action.payload;
    },
    setWindow: (state, action: PayloadAction<MetricsWindow>) => {
      state.window = action.payload;
    },
    togglePinnedPlace: (state, action: PayloadAction<string>) => {
      const placeId = action.payload;
      state.pinnedPlaceIds = state.pinnedPlaceIds.includes(placeId)
        ? state.pinnedPlaceIds.filter($ => $ !== placeId)
        : [...state.pinnedPlaceIds, placeId];
    },
    setPinnedPlace: (state, action: PayloadAction<{ placeId: string; pinned: boolean }>) => {
      const { placeId, pinned } = action.payload;
      const others = state.pinnedPlaceIds.filter($ => $ !== placeId);
      state.pinnedPlaceIds = pinned ? [...others, placeId] : others;
    },
  },
  extraReducers: builder => {
    builder
      .addCase(accountChanged, () => initialState)
      .addCase(loadPinnedPlaces.pending, (state, action) => {
        state.watchlist = {
          address: action.meta.arg.address.toLowerCase(),
          status: 'loading',
          requestId: action.meta.requestId,
        };
      })
      .addCase(loadPinnedPlaces.fulfilled, (state, action) => {
        if (action.meta.requestId !== state.watchlist.requestId) return;
        state.pinnedPlaceIds = action.payload;
        state.watchlist.status = 'succeeded';
      })
      .addCase(loadPinnedPlaces.rejected, (state, action) => {
        if (action.meta.requestId !== state.watchlist.requestId) return;
        state.watchlist.status = 'failed';
      })
      .addCase(fetchAnalytics.pending, (state, action) => {
        state.status = 'loading';
        state.error = null;
        state.requestId = action.meta.requestId;
      })
      .addCase(fetchAnalytics.fulfilled, (state, action) => {
        if (action.meta.requestId !== state.requestId) return;
        state.exportedAt = action.payload.exportedAt;
        state.places = action.payload.places;
        state.metricsByPlaceId = action.payload.metricsByPlaceId;
        state.status = 'succeeded';
        state.error = null;
      })
      .addCase(fetchAnalytics.rejected, (state, action) => {
        if (action.meta.requestId !== state.requestId) return;
        state.status = 'failed';
        state.error = action.error.message || 'Failed to fetch analytics';
      });
  },
});

/**
 * Pins or unpins a place, then saves the account's whole list. The list in the store is the one
 * saved, pins of places the last fetch did not return included, so a failed or empty fetch never
 * removes them. Nothing is saved before the stored list has loaded: that save would replace it.
 * A stored list that could not be read is read again first. What the click asks for is taken
 * from the list on screen before that read (Pin on a row that showed unpinned pins it, even
 * when the reread list already has it), so a reread never turns the click into its opposite.
 * The thunk rejects when the pin is
 * not on disk (the list still unread, or the save failed), which the snackbar slice reports.
 * Writes run one after another (`setConfig`), so the last toggle's list is the one on disk.
 */
export const togglePin = createAsyncThunk(
  'placeAnalytics/togglePin',
  async (placeId: string, { dispatch, getState }) => {
    const pinned = !getState().placeAnalytics.pinnedPlaceIds.includes(placeId);
    const unread = getState().placeAnalytics.watchlist;
    if (unread.status === 'failed' && unread.address) {
      await dispatch(loadPinnedPlaces({ address: unread.address }));
    }
    dispatch(slice.actions.setPinnedPlace({ placeId, pinned }));
    const { pinnedPlaceIds, watchlist } = getState().placeAnalytics;
    if (watchlist.status === 'failed') throw new Error('The stored watchlist could not be read');
    if (watchlist.status !== 'succeeded' || !watchlist.address) return;
    await dispatch(
      savePinnedPlaces({ address: watchlist.address, placeIds: pinnedPlaceIds }),
    ).unwrap();
  },
);

const getPlaceAnalyticsState = (state: AppState) => state.placeAnalytics;

/**
 * Places to render: matching the search query, pinned ones first, then sorted.
 *
 * The whole list arrives in one response, so filtering and sorting happen here.
 */
const getVisiblePlaces = createSelector(getPlaceAnalyticsState, analytics =>
  sortPlaces(
    analytics.places.flatMap(place => {
      const metrics = analytics.metricsByPlaceId[place.placeId];
      return metrics ? [toSummary(metrics, analytics.window, place)] : [];
    }),
    analytics.searchQuery,
    analytics.sortBy,
    analytics.pinnedPlaceIds,
  ),
);

/** The metrics for one scene, or `undefined` if it is not in the snapshot. */
export const getPlaceMetrics = (state: AppState, placeId: string): LocationMetrics | undefined =>
  state.placeAnalytics.metricsByPlaceId[placeId];

export const getPlace = (state: AppState, placeId: string): AnalyticsPlace | undefined =>
  state.placeAnalytics.places.find(place => place.placeId === placeId);

export const selectors = { getVisiblePlaces, getPlaceMetrics, getPlace };
export const actions = {
  ...slice.actions,
  fetchAnalytics,
  loadPinnedPlaces,
  savePinnedPlaces,
  togglePin,
};
export const reducer = slice.reducer;
