import { describe, expect, it } from 'vitest';

import { playtimeAmount } from './playtime';

describe('playtimeAmount', () => {
  describe('when the total is an hour or less', () => {
    it.each([
      [0.2, 0],
      [4.77, 5],
      [60, 60],
      [60.4, 60],
    ])('should read %s minutes as %s whole minutes', (minutes, value) => {
      expect(playtimeAmount(minutes)).toEqual({ unit: 'minutes', value });
    });
  });

  describe('when the total is over an hour', () => {
    it.each([
      [60.6, 1],
      [61, 1],
      [90, 1.5],
      [5120, 85.3],
      [8829.64, 147.2],
      [204072, 3401.2],
    ])('should read %s minutes as %s hours, to one decimal', (minutes, value) => {
      expect(playtimeAmount(minutes)).toEqual({ unit: 'hours', value });
    });
  });
});
