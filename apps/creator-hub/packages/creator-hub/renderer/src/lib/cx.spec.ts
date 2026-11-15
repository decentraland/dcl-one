import { describe, expect, it } from 'vitest';

import { cx } from './cx';

describe('cx', () => {
  describe('when it gets strings and numbers', () => {
    it('should join them with a space', () => {
      expect(cx('Button', 'primary', 2)).toBe('Button primary 2');
    });
  });

  describe('when it gets falsy values', () => {
    it('should leave them out', () => {
      expect(cx('Button', undefined, null, false, '', 0, true, 'last')).toBe('Button last');
    });
  });

  describe('when it gets an object', () => {
    it('should give the keys that have a truthy value', () => {
      expect(cx('Button', { active: true, disabled: false, large: 1, small: undefined })).toBe(
        'Button active large',
      );
    });
  });

  describe('when it gets an array', () => {
    it('should give the names of the array, at each depth', () => {
      expect(cx(['a', { b: true, c: false }, ['d', [null, 'e']]], [], 'f')).toBe('a b d e f');
    });
  });

  describe('when it gets no value', () => {
    it('should give an empty string', () => {
      expect(cx()).toBe('');
    });
  });
});
