import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import Button from '@ui/atoms/HubButton';
import { SelectOption } from '@ui/atoms/Select';
import { Tab, Tabs } from '@ui/atoms/Tabs';
import Text from '@ui/atoms/Text';
import Tooltip from '@ui/atoms/Tooltip';

import { FileDownloadOutlinedIcon as DownloadIcon } from '/@/components/Icons/material';

import { MetricsWindow, PlaceAccess } from '/shared/types/place-analytics';

import { useDispatch, useSelector } from '#store';
import { t } from '/@/modules/store/translation/utils';
import { actions as placeAnalyticsActions, selectors } from '/@/modules/store/placeAnalytics';
import {
  hasNoData,
  toEngagement,
  toOverview,
  toRetention,
  toVisits,
} from '/@/lib/placeAnalytics.adapter';
import { useAuth } from '/@/hooks/useAuth';

import { Container } from '../Container';
import { Loader } from '../Loader';
import { Navbar, NavbarItem } from '../Navbar';
import { Select } from '../Select';
import type { SelectChangeEvent } from '../Select/component';
import { SignInCard } from '../SignInCard';
import { Title } from '../Title';
import { formatExportDate } from '../AnalyticsPage/utils';

import { OverviewTab } from './OverviewTab';
import { PlaceCard } from './PlaceCard';
import { EngagementTab } from './EngagementTab';
import { RetentionTab } from './RetentionTab';
import { VisitsTab } from './VisitsTab';

import './styles.css';

const WINDOW_OPTIONS: Array<{ label: string; value: MetricsWindow }> = [
  { label: t('analytics.detail.window.last_30_days'), value: MetricsWindow.LAST_30_DAYS },
  { label: t('analytics.detail.window.last_60_days'), value: MetricsWindow.LAST_60_DAYS },
];

const WINDOW_LABEL_ID = 'analytics-window-label';

const TABS = [
  { value: 'overview', label: t('analytics.detail.tabs.overview') },
  { value: 'retention', label: t('analytics.detail.tabs.retention') },
  { value: 'visits', label: t('analytics.detail.tabs.visits') },
  { value: 'engagement', label: t('analytics.detail.tabs.engagement') },
];

type TabValue = 'overview' | 'retention' | 'visits' | 'engagement';

export function AnalyticsDetailPage() {
  const { placeId = '' } = useParams<{ placeId: string }>();
  const { isSignedIn, isSigningIn, signIn } = useAuth();
  const {
    window: metricsWindow,
    exportedAt,
    status,
    error,
  } = useSelector(state => state.placeAnalytics);
  const place = useSelector(state => selectors.getPlace(state, placeId));
  const metrics = useSelector(state => selectors.getPlaceMetrics(state, placeId));
  const [tab, setTab] = useState<TabValue>('overview');
  const dispatch = useDispatch();
  const navigate = useNavigate();

  const isLoading = status === 'idle' || status === 'loading';

  // One batched request answers every tab, so a deep link only has to make sure
  // the snapshot is loaded.
  useEffect(() => {
    if (isSignedIn && status === 'idle') dispatch(placeAnalyticsActions.fetchAnalytics());
  }, [isSignedIn, status, dispatch]);

  const handleBack = useCallback(() => navigate('/analytics'), [navigate]);

  const handleTabChange = useCallback(
    (value: string) => {
      setTab(value as TabValue);
    },
    [setTab],
  );

  const handleWindowChange = useCallback(
    (e: SelectChangeEvent<MetricsWindow>) => {
      dispatch(placeAnalyticsActions.setWindow(e.target.value as MetricsWindow));
    },
    [dispatch],
  );

  const projections = useMemo(
    () =>
      metrics
        ? {
            overview: toOverview(metrics, metricsWindow),
            retention: toRetention(metrics, metricsWindow, exportedAt),
            visits: toVisits(metrics, metricsWindow),
            engagement: toEngagement(metrics, metricsWindow),
            isEmpty: hasNoData(metrics),
          }
        : null,
    [metrics, metricsWindow, exportedAt],
  );

  const title = place
    ? t('analytics.detail.title', { name: place.name })
    : t('analytics.header.title');

  return (
    <main className="AnalyticsDetailPage">
      <Navbar active={NavbarItem.ANALYTICS} />
      <Container>
        <div className="DetailHeader">
          <div className="HeaderTitle">
            <Title
              value={title}
              onBack={handleBack}
            />
            {exportedAt && (
              <Text
                variant="body2"
                className="ExportedAt"
              >
                {t('analytics.exported_at', { date: formatExportDate(exportedAt) })}
              </Text>
            )}
          </div>
          {/* Enabled once there is an analytics API to export from. */}
          <Tooltip
            className="ExportAction"
            label={t('analytics.actions.export_unavailable')}
            side="bottom"
            portal
            keepInViewport
            snap
            tipClassName="AnalyticsTip"
          >
            <Button
              variant="outline"
              tone="neutral"
              startIcon={<DownloadIcon />}
              disabled
            >
              {t('analytics.actions.export')}
            </Button>
          </Tooltip>
        </div>
        {/* Before the loading branch: with nothing in flight, `idle` is not a wait. */}
        {!isSignedIn && !isSigningIn ? (
          <SignInCard
            onClickSignIn={signIn}
            title={t('analytics.sign_in.title')}
          />
        ) : isLoading ? (
          <Loader size={70} />
        ) : status === 'failed' ? (
          <div className="ErrorContainer">
            <Text variant="h5">{t('analytics.detail.error.title')}</Text>
            <Text variant="body1">{error}</Text>
          </div>
        ) : !place || !projections ? (
          <div className="ErrorContainer">
            <Text variant="h5">{t('analytics.detail.not_found.title')}</Text>
            <Text variant="body1">{t('analytics.detail.not_found.description')}</Text>
          </div>
        ) : (
          <div className="Content">
            <PlaceCard
              place={{
                placeId,
                name: place.name,
                thumbnail: place.thumbnail,
                likeRate: null,
                access: PlaceAccess.PUBLIC,
                publishedIn: place.publishedIn,
                location: place.location,
                lastPublishedBy: null,
                lastUpdatedAt: place.lastUpdatedAt,
              }}
            />
            <div className="Metrics">
              <div className="TabsBar">
                <Tabs
                  className="Tabs"
                  value={tab}
                  onChange={handleTabChange}
                >
                  {TABS.map(option => (
                    <Tab
                      className="Tab"
                      key={option.value}
                      value={option.value}
                      label={option.label}
                    />
                  ))}
                </Tabs>
                <div className="DateRange">
                  <Text
                    variant="body1"
                    className="DateRangeLabel"
                    id={WINDOW_LABEL_ID}
                  >
                    {t('analytics.detail.window.title')}
                  </Text>
                  <Select
                    labelId={WINDOW_LABEL_ID}
                    value={metricsWindow}
                    onChange={handleWindowChange}
                  >
                    {WINDOW_OPTIONS.map(option => (
                      <SelectOption
                        key={option.value}
                        value={option.value}
                      >
                        {option.label}
                      </SelectOption>
                    ))}
                  </Select>
                </div>
              </div>
              {/*
               * An empty bag means either "no rows in today's export" or "this
               * wallet may not read it" — deliberately indistinguishable, and
               * never an error.
               */}
              {projections.isEmpty ? (
                <div className="EmptyContainer">
                  <Text variant="h5">{t('analytics.no_data_yet')}</Text>
                  <Text variant="body1">{t('analytics.no_data_yet_description')}</Text>
                </div>
              ) : (
                <>
                  {tab === 'overview' && <OverviewTab overview={projections.overview} />}
                  {tab === 'retention' && (
                    <RetentionTab
                      retention={projections.retention}
                      window={metricsWindow}
                    />
                  )}
                  {tab === 'visits' && <VisitsTab visits={projections.visits} />}
                  {tab === 'engagement' && <EngagementTab engagement={projections.engagement} />}
                </>
              )}
            </div>
          </div>
        )}
      </Container>
    </main>
  );
}
