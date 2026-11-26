import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { settings as settingsPreload } from '#preload';
import { AuthServerProvider } from '/@/lib/auth';
import type { LocationMetrics } from '/@/lib/metricsApi';
import { fetchAnalytics as fetchAnalyticsSnapshot } from '/@/lib/placeAnalytics';
import { useAuth } from '/@/hooks/useAuth';

import { createTestStore, type TestStore } from '../../../tests/utils/testStore';
import { AnalyticsPage } from './component';

/** The real store module boots the app on import; bind the hooks to the test store. */
vi.mock('#store', async () => {
  const { useDispatch, useSelector } = await import('react-redux');
  return { useDispatch, useSelector };
});

/** The navbar drags in the whole modal tree; none of it is what these tests exercise. */
vi.mock('../Navbar', () => ({ Navbar: () => null, NavbarItem: { ANALYTICS: 'analytics' } }));

vi.mock('/@/lib/placeAnalytics', () => ({ fetchAnalytics: vi.fn() }));
vi.mock('/@/lib/auth', () => ({ AuthServerProvider: { getAccount: vi.fn() } }));
vi.mock('/@/hooks/useAuth', () => ({ useAuth: vi.fn() }));

/** The scene lists the thunk reads before asking for metrics; empty is enough here. */
vi.mock('/@/modules/store/management/utils', async importOriginal => ({
  ...(await importOriginal<object>()),
  fetchAllDeployedWorlds: vi.fn(() => Promise.resolve([])),
}));
vi.mock('/@/modules/store/land', async importOriginal => ({
  ...(await importOriginal<object>()),
  fetchLandList: vi.fn(() => () => {
    const land = { land: [] };
    return Object.assign(Promise.resolve(land), { unwrap: () => Promise.resolve(land) });
  }),
}));

/** With a wallet the page also refreshes its names; that request is not what these tests check. */
vi.mock('/@/modules/store/ens', async importOriginal => ({
  ...(await importOriginal<object>()),
  fetchENSList: vi.fn(() => () => Promise.resolve()),
}));

describe('AnalyticsPage', () => {
  let store: TestStore;
  const signIn = vi.fn();

  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store as never}>
      <MemoryRouter>{children}</MemoryRouter>
    </Provider>
  );

  const setAuth = (isSignedIn: boolean, isSigningIn = false, wallet?: string) =>
    vi.mocked(useAuth).mockReturnValue({ isSignedIn, isSigningIn, signIn, wallet } as any);

  beforeEach(() => {
    store = createTestStore();
    vi.mocked(AuthServerProvider.getAccount).mockReturnValue('0x123abc');
    vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue({
      exportedAt: '2026-08-12T00:17:01.099Z',
      places: [],
      metricsByPlaceId: {},
    });
  });

  describe('when the creator is not signed in', () => {
    beforeEach(() => {
      setAuth(false);
    });

    /*
     * `idle` used to count as loading while the fetch waited on a sign-in that
     * never came, which left a spinner spinning forever.
     */
    it('should offer to sign in rather than spin forever', () => {
      render(<AnalyticsPage />, { wrapper });

      expect(screen.getByRole('button', { name: /sign in/i })).toBeTruthy();
    });

    it('should not ask the service for anything', () => {
      render(<AnalyticsPage />, { wrapper });

      expect(fetchAnalyticsSnapshot).not.toHaveBeenCalled();
    });
  });

  describe('when a sign-in is in flight', () => {
    beforeEach(() => {
      setAuth(false, true);
    });

    it('should wait rather than prompt again', () => {
      render(<AnalyticsPage />, { wrapper });

      expect(screen.queryByRole('button', { name: /sign in/i })).toBeNull();
    });
  });

  describe('when the creator is signed in', () => {
    beforeEach(() => {
      setAuth(true);
    });

    it('should load the snapshot', async () => {
      render(<AnalyticsPage />, { wrapper });

      await waitFor(() => expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1));
    });

    it('should not reload it when the page is revisited', async () => {
      const first = render(<AnalyticsPage />, { wrapper });
      await waitFor(() => expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1));
      first.unmount();

      render(<AnalyticsPage />, { wrapper });

      await waitFor(() => expect(store.getState().placeAnalytics.status).toBe('succeeded'));
      expect(fetchAnalyticsSnapshot).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the signed-in wallet has pinned places stored', () => {
    const WALLET = '0x00000000000000000000000000000000000000AA';
    const PINNED = 'world:pinned.dcl.eth@0,0';
    const OTHER = 'world:other.dcl.eth@0,0';
    const metrics = {
      location_key: 'unused',
      x: 0,
      y: 0,
      builder_project_id: null,
      metrics: { unique_visits_60d: [{ series: 'all', period: null, value: 1 }] },
    } as LocationMetrics;
    const place = (placeId: string, name: string) => ({
      placeId,
      name,
      thumbnail: 'thumb.png',
      location: { world: name, x: 0, y: 0 },
      publishedIn: name,
      lastUpdatedAt: null,
    });

    const rowNames = (container: HTMLElement) =>
      [...container.querySelectorAll('tbody tr')].map(row => row.getAttribute('aria-label'));

    beforeEach(() => {
      setAuth(true, false, WALLET);
      vi.mocked(AuthServerProvider.getAccount).mockReturnValue(WALLET);
      vi.mocked(fetchAnalyticsSnapshot).mockResolvedValue({
        exportedAt: '2026-08-12T00:17:01.099Z',
        places: [place(OTHER, 'a-other.dcl.eth'), place(PINNED, 'z-pinned.dcl.eth')],
        metricsByPlaceId: { [OTHER]: metrics, [PINNED]: metrics },
      });
    });

    it("should read that wallet's pins and list them first", async () => {
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockResolvedValue([PINNED]);

      const { container } = render(<AnalyticsPage />, { wrapper });

      await waitFor(() => expect(rowNames(container)).toHaveLength(2));
      expect(settingsPreload.getAnalyticsWatchlist).toHaveBeenCalledWith(WALLET.toLowerCase());
      expect(rowNames(container)).toEqual(['z-pinned.dcl.eth', 'a-other.dcl.eth']);
    });

    it('should still show the list when the stored pins cannot be read', async () => {
      vi.mocked(settingsPreload.getAnalyticsWatchlist).mockRejectedValue(new Error('no config'));

      const { container } = render(<AnalyticsPage />, { wrapper });

      await waitFor(() => expect(rowNames(container)).toHaveLength(2));
      expect(store.getState().placeAnalytics.watchlist.status).toBe('failed');
    });
  });
});
