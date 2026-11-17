import { cleanup, render, screen } from '@testing-library/react';
import { flatten } from 'flat';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';

import type { PlaceOverviewMetrics } from '/shared/types/place-analytics';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { OverviewTab } from './component';

const OVERVIEW: PlaceOverviewMetrics = {
  totalVisits: 1284,
  uniqueVisits: 412,
  concurrentUsers: 1.5,
  peakConcurrentUsers: 9,
  newPlayers: 37,
  day3Retention: 21.4,
  day7Retention: 12,
  medianPlaytime: 4.25,
  totalPlaytime: 8829.64,
  afkTime: 1.5,
  desktopUsers: 300,
  mobileUsers: 112,
};

const shownValue = (label: string) =>
  screen.getByText(label).closest('.MetricDisplay')?.querySelector('.MetricValue')?.textContent;

describe('OverviewTab', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  describe('when showing the total playtime', () => {
    it.each([
      [8829.64, '8.83K min'],
      [4.77, '5 min'],
      [0.83, '1 min'],
    ])('should show %s minutes as %j', (totalPlaytime, shown) => {
      render(<OverviewTab overview={{ ...OVERVIEW, totalPlaytime }} />);

      expect(shownValue('Total Playtime')).toBe(shown);
    });

    it('should show "-" without a unit when the total is unknown', () => {
      render(<OverviewTab overview={{ ...OVERVIEW, totalPlaytime: null }} />);

      expect(shownValue('Total Playtime')).toBe('-');
    });

    it('should pair it with Total Visits on the traffic card first row, adding no row', () => {
      render(<OverviewTab overview={OVERVIEW} />);

      const [traffic] = document.querySelectorAll('.MetricsCard');
      const cells = [...traffic.children].flatMap(child =>
        child.classList.contains('MetricsGrid') ? [...child.children] : [child],
      );
      expect(cells.map(cell => cell.querySelector('.MetricLabel .Label')?.textContent)).toEqual([
        'Total Visits',
        'Total Playtime',
        'Unique Visits',
        'New Decentraland Players',
        'Concurrent Users',
        'Peak Concurrent Users',
      ]);
      expect(cells.every(cell => cell.parentElement?.classList.contains('MetricsGrid'))).toBe(true);
    });
  });
});
