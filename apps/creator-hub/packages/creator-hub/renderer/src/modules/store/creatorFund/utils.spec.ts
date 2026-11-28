import { describe, expect, it } from 'vitest';

import { cycleDayRange, formatCycleDays, formatCycleLabel } from './utils';

const cycle = (start_at: string, end_at: string) => ({ start_at, end_at });

describe('formatCycleLabel', () => {
  describe('when the cycle ends at midnight of the next day', () => {
    it('should label the last included day, since end_at is exclusive', () => {
      expect(formatCycleLabel(cycle('2026-10-01T00:00:00Z', '2026-10-16T00:00:00Z'), 'en-US')).toBe(
        'Oct 1 - Oct 15, 2026',
      );
    });

    it('should step back into the previous month when end_at is the 1st', () => {
      expect(formatCycleLabel(cycle('2026-10-16T00:00:00Z', '2026-11-01T00:00:00Z'), 'en-US')).toBe(
        'Oct 16 - Oct 31, 2026',
      );
    });
  });

  describe('when the cycle spans two months', () => {
    it('should name both months', () => {
      expect(formatCycleLabel(cycle('2026-10-20T00:00:00Z', '2026-11-04T00:00:00Z'), 'en-US')).toBe(
        'Oct 20 - Nov 3, 2026',
      );
    });
  });

  describe('when the cycle spans two years', () => {
    it('should name both years', () => {
      expect(formatCycleLabel(cycle('2026-12-20T00:00:00Z', '2027-01-04T00:00:00Z'), 'en-US')).toBe(
        'Dec 20, 2026 - Jan 3, 2027',
      );
    });
  });

  describe('when the locale is not English', () => {
    it('should format the days in that locale', () => {
      expect(formatCycleLabel(cycle('2026-10-01T00:00:00Z', '2026-10-16T00:00:00Z'), 'es')).toBe(
        '1 oct - 15 oct 2026',
      );
    });
  });
});

describe('formatCycleDays', () => {
  describe('when the cycle is within one year', () => {
    it('should label its days without the year', () => {
      expect(formatCycleDays(cycle('2026-10-01T00:00:00Z', '2026-10-16T00:00:00Z'), 'en-US')).toBe(
        'Oct 1 - Oct 15',
      );
    });
  });

  describe('when the cycle spans two years', () => {
    it('should still leave the years out', () => {
      expect(formatCycleDays(cycle('2026-12-20T00:00:00Z', '2027-01-04T00:00:00Z'), 'en-US')).toBe(
        'Dec 20 - Jan 3',
      );
    });
  });
});

describe('cycleDayRange', () => {
  describe('when the cycle starts and ends at 03:00 UTC', () => {
    it('should span the UTC day it starts on through the day before it ends', () => {
      expect(cycleDayRange(cycle('2026-11-15T03:00:00.000Z', '2026-12-01T03:00:00.000Z'))).toEqual({
        from: '2026-11-15',
        to: '2026-11-30',
      });
    });
  });

  describe('when the cycle ends at midnight UTC', () => {
    it('should leave out the day end_at falls on, since end_at is exclusive', () => {
      expect(cycleDayRange(cycle('2026-10-01T00:00:00Z', '2026-10-16T00:00:00Z'))).toEqual({
        from: '2026-10-01',
        to: '2026-10-15',
      });
    });
  });

  describe('when the cycle ends on the first day of a year', () => {
    it('should end on the last day of the year before', () => {
      expect(cycleDayRange(cycle('2026-12-16T03:00:00Z', '2027-01-01T03:00:00Z'))).toEqual({
        from: '2026-12-16',
        to: '2026-12-31',
      });
    });
  });
});
