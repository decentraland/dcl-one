import { describe, expect, it } from 'vitest';

import type { LocationMetrics } from '/@/lib/metricsApi';

import { formatUsdc, totalPlaytimeSeconds30d } from './format';

const location = (values: Record<string, number>): LocationMetrics => ({
  x: 0,
  y: 0,
  location_key: '0,0',
  builder_project_id: null,
  metrics: Object.fromEntries(
    Object.entries(values).map(([name, value]) => [
      name,
      [
        { series: 'all', period: null, value },
        { series: 'desktop', period: null, value: value / 2 },
      ],
    ]),
  ),
});

describe('totalPlaytimeSeconds30d', () => {
  it('should multiply the whole-population average playtime by the visits', () => {
    expect(
      totalPlaytimeSeconds30d(location({ avg_playtime_seconds_30d: 95.5, unique_visits_30d: 4 })),
    ).toBe(382);
  });

  it.each([
    ['the average', { unique_visits_30d: 4 }],
    ['the visits', { avg_playtime_seconds_30d: 95.5 }],
  ])('should be null without %s', (_, values) => {
    expect(totalPlaytimeSeconds30d(location(values))).toBeNull();
  });

  it('should be null without metrics', () => {
    expect(totalPlaytimeSeconds30d(undefined)).toBeNull();
  });
});

describe('formatUsdc', () => {
  describe('when the amount is a decimal string', () => {
    it.each([
      ['22.00', '22'],
      ['22', '22'],
      ['1842', '1,842'],
      ['1842.50', '1,842.50'],
      ['1842.5', '1,842.50'],
      ['0.49', '0.49'],
      ['0.004', '0.004'],
      ['1.005', '1.005'],
      ['9.999999', '9.999999'],
      ['12345678901234567.89', '12,345,678,901,234,567.89'],
    ])('should show %s as %s, never rounded', (amount, shown) => {
      expect(formatUsdc(amount, 'en')).toBe(shown);
    });
  });

  describe('when the amount is not a plain decimal', () => {
    it.each([[''], ['abc'], ['1e3'], ['n/a']])('should show %j as sent, never NaN', amount => {
      expect(formatUsdc(amount, 'en')).toBe(amount);
    });
  });
});
