import { useCallback, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { SelectOption } from '@ui/atoms/Select';
import Text from '@ui/atoms/Text';

import { SortBy } from '/shared/types/place-analytics';

import { useDispatch, useSelector } from '#store';
import { t } from '/@/modules/store/translation/utils';
import { actions as placeAnalyticsActions, selectors } from '/@/modules/store/placeAnalytics';
import { fetchENSList } from '/@/modules/store/ens';
import { useAuth } from '/@/hooks/useAuth';

import emptyAnalytics from '/assets/images/analytics-empty.svg';

import { Container } from '../Container';
import { FiltersBar } from '../FiltersBar';
import { Loader } from '../Loader';
import { Navbar, NavbarItem } from '../Navbar';
import { Search } from '../Search';
import { Select } from '../Select';
import type { SelectChangeEvent } from '../Select/component';
import { SignInCard } from '../SignInCard';

import { PlacesTable } from './PlacesTable';
import { formatExportDate } from './utils';

import './styles.css';

const SORT_OPTIONS: Array<{ label: string; value: SortBy }> = [
  { label: t('analytics.list.sort.name_asc'), value: SortBy.NAME_ASC },
  { label: t('analytics.list.sort.name_desc'), value: SortBy.NAME_DESC },
  { label: t('analytics.list.sort.most_visits'), value: SortBy.MOST_VISITS },
];

export function AnalyticsPage() {
  const { isSignedIn, isSigningIn, signIn, wallet } = useAuth();
  const { places, pinnedPlaceIds, sortBy, searchQuery, exportedAt, status, error } = useSelector(
    state => state.placeAnalytics,
  );
  const visiblePlaces = useSelector(selectors.getVisiblePlaces);
  const dispatch = useDispatch();
  const navigate = useNavigate();

  const isLoading = status === 'loading' || status === 'idle';

  // One batched request answers the whole page, so revisiting it re-reads the
  // snapshot already in the store rather than asking again.
  useEffect(() => {
    if (isSignedIn && status === 'idle') dispatch(placeAnalyticsActions.fetchAnalytics());
  }, [isSignedIn, status, dispatch]);

  // Names can change outside the app (a purchase, a granted deploy permission), so the
  // sign-in-time fetch goes stale. Refresh in the background on every visit to this tab.
  useEffect(() => {
    if (wallet) {
      dispatch(fetchENSList({ address: wallet }));
    }
  }, [wallet, dispatch]);

  const handleSortChange = useCallback(
    (e: SelectChangeEvent<SortBy>) => {
      dispatch(placeAnalyticsActions.setSortBy(e.target.value as SortBy));
    },
    [dispatch],
  );

  const handleSearch = useCallback(
    (value: string) => {
      dispatch(placeAnalyticsActions.setSearchQuery(value));
    },
    [dispatch],
  );

  const handleTogglePin = useCallback(
    (placeId: string) => {
      dispatch(placeAnalyticsActions.togglePinnedPlace(placeId));
    },
    [dispatch],
  );

  const handleSelectPlace = useCallback(
    (placeId: string) => navigate(`/analytics/${placeId}`),
    [navigate],
  );

  return (
    <main className="AnalyticsPage">
      <Navbar active={NavbarItem.ANALYTICS} />
      <Container>
        <Text variant="h3">{t('analytics.header.title')}</Text>
        {/* Before the loading branch: with nothing in flight, `idle` is not a wait. */}
        {!isSignedIn && !isSigningIn ? (
          <SignInCard
            onClickSignIn={signIn}
            title={t('analytics.sign_in.title')}
          />
        ) : isLoading ? (
          <Loader size={70} />
        ) : status === 'failed' ? (
          /* Without this a failed request reads as "you have no Places yet". */
          <div className="EmptyContainer">
            <Text variant="h5">{t('analytics.error.title')}</Text>
            <Text
              variant="body1"
              className="Description"
            >
              {error}
            </Text>
          </div>
        ) : places.length === 0 ? (
          <div className="EmptyContainer">
            <img
              src={emptyAnalytics}
              alt=""
            />
            <Text variant="h5">{t('analytics.empty_list.title')}</Text>
            <Text
              variant="body1"
              className="Description"
            >
              {t('analytics.empty_list.description')}
            </Text>
          </div>
        ) : (
          <>
            <FiltersBar className="FiltersBar">
              <Text variant="h6">
                {t('analytics.list.places', { count: visiblePlaces.length })}
              </Text>
              <>
                {/* The export's own stamp: an "as of" date is a different answer from silence. */}
                {exportedAt && (
                  <Text
                    variant="body2"
                    className="ExportedAt FilterText"
                  >
                    {t('analytics.exported_at', { date: formatExportDate(exportedAt) })}
                  </Text>
                )}
                <Text
                  variant="body1"
                  className="FilterText"
                >
                  {t('analytics.list.sort.title')}
                </Text>
                <Select
                  value={sortBy}
                  onChange={handleSortChange}
                >
                  {SORT_OPTIONS.map(option => (
                    <SelectOption
                      key={option.value}
                      value={option.value}
                    >
                      {option.label}
                    </SelectOption>
                  ))}
                </Select>
                <Search
                  placeholder={t('analytics.list.search')}
                  defaultValue={searchQuery}
                  onChange={handleSearch}
                />
              </>
            </FiltersBar>
            {visiblePlaces.length === 0 ? (
              <div className="EmptyContainer">
                <Text variant="h5">{t('analytics.empty_list.search.title')}</Text>
                <Text
                  variant="body1"
                  className="Description"
                >
                  {t('analytics.empty_list.search.description')}
                </Text>
              </div>
            ) : (
              <PlacesTable
                places={visiblePlaces}
                pinnedPlaceIds={pinnedPlaceIds}
                onTogglePin={handleTogglePin}
                onSelectPlace={handleSelectPlace}
              />
            )}
          </>
        )}
      </Container>
    </main>
  );
}
