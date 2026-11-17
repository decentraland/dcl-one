import { useCallback } from 'react';
import Text from '@ui/atoms/Text';

import {
  PushPinIcon,
  FileDownloadOutlinedIcon as DownloadIcon,
} from '/@/components/Icons/material';

import type { PlaceAnalyticsSummary } from '/shared/types/place-analytics';

import { t } from '/@/modules/store/translation/utils';

import { Dropdown } from '../../Dropdown';
import {
  formatCount,
  formatDecimal,
  formatMinutes,
  formatPercentage,
  getRetentionColor,
} from '../utils';

import './styles.css';

const PALETTE_COLORS: Record<string, string> = {
  'success.main': 'var(--success)',
  'primary.main': 'var(--primary)',
};

type Props = {
  places: PlaceAnalyticsSummary[];
  pinnedPlaceIds: string[];
  onTogglePin: (placeId: string) => void;
  onSelectPlace: (placeId: string) => void;
};

export function PlacesTable({ places, pinnedPlaceIds, onTogglePin, onSelectPlace }: Props) {
  const getDropdownOptions = useCallback(
    (place: PlaceAnalyticsSummary) => [
      {
        text: pinnedPlaceIds.includes(place.placeId)
          ? t('analytics.list.actions.unpin_from_watchlist')
          : t('analytics.list.actions.pin_to_watchlist'),
        icon: <PushPinIcon />,
        handler: () => onTogglePin(place.placeId),
      },
      {
        text: t('analytics.actions.export'),
        icon: <DownloadIcon />,
        // Enabled once there is an analytics API to export from.
        disabled: true,
        handler: () => undefined,
      },
    ],
    [pinnedPlaceIds, onTogglePin],
  );

  return (
    <table className="PlacesTable">
      <thead>
        <tr>
          <th scope="col">{t('analytics.list.columns.place')}</th>
          <th scope="col">{t('analytics.list.columns.total_visits')}</th>
          {/* new_users: awaiting a first-time-visitor metric.
          <th scope="col">{t('analytics.list.columns.new_users')}</th> */}
          <th scope="col">{t('analytics.list.columns.day_7_retention')}</th>
          {/* revenue: awaiting a revenue metric.
          <th scope="col">{t('analytics.list.columns.revenue')}</th> */}
          <th scope="col">{t('analytics.list.columns.median_playtime')}</th>
          <th scope="col">{t('analytics.list.columns.concurrent_users')}</th>
          <th className="ActionsCell" />
        </tr>
      </thead>
      <tbody>
        {places.map(place => (
          <tr
            key={place.placeId}
            tabIndex={0}
            role="button"
            aria-label={place.name}
            onClick={() => onSelectPlace(place.placeId)}
            onKeyDown={e => {
              // Keys from the options button and its menu (in a portal, but in this React tree) are not for the row.
              if (e.target !== e.currentTarget) return;
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onSelectPlace(place.placeId);
              }
            }}
          >
            <td>
              <div className="PlaceCell">
                <img
                  className="Thumbnail"
                  src={place.thumbnail}
                  alt=""
                />
                <Text
                  variant="body2"
                  className="PlaceName"
                >
                  {place.name}
                </Text>
                {pinnedPlaceIds.includes(place.placeId) && (
                  <PushPinIcon
                    className="PinnedIcon"
                    fontSize="small"
                    titleAccess={t('analytics.list.pinned')}
                  />
                )}
              </div>
            </td>
            {place.hasNoData ? (
              /*
               * The API returned nothing for this scene — either it holds no rows
               * in today's export or this wallet may not read it, deliberately
               * indistinguishable. Neither is an error.
               */
              <td
                colSpan={4}
                className="NoDataCell"
              >
                <Text variant="body2">{t('analytics.no_data_yet')}</Text>
              </td>
            ) : (
              <>
                <td>{formatCount(place.totalVisits)}</td>
                {/* new_users: awaiting a first-time-visitor metric.
                <td>{formatCount(place.newUsers)}</td> */}
                <td>
                  <Text
                    variant="body2"
                    style={{ color: PALETTE_COLORS[getRetentionColor(place.day7Retention) ?? ''] }}
                  >
                    {formatPercentage(place.day7Retention)}
                  </Text>
                </td>
                {/* revenue: awaiting a revenue metric.
                <td>
                  <ManaValue>{formatRevenue(place.revenue)}</ManaValue>
                </td> */}
                <td>{formatMinutes(place.medianPlaytime)}</td>
                <td>{formatDecimal(place.concurrentUsers)}</td>
              </>
            )}
            <td className="ActionsCell">
              <Dropdown
                label={t('menu.options_for', { name: place.name })}
                options={getDropdownOptions(place)}
              />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
