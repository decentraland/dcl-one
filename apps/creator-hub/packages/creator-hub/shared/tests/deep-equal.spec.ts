import { describe, expect, it } from 'vitest';

import { deepEqual } from '../deep-equal';

describe('deepEqual', () => {
  describe('when the two values have the same content', () => {
    it.each([
      ['numbers', 1, 1],
      ['NaN values', NaN, NaN],
      ['empty objects', {}, {}],
      ['objects with the keys in another order', { a: 1, b: [1, 2] }, { b: [1, 2], a: 1 }],
      ['arrays of objects', [{ a: { b: null } }, 'x'], [{ a: { b: null } }, 'x']],
      ['keys with the value undefined', { a: undefined }, { a: undefined }],
      ['dates', new Date(86400000), new Date(86400000)],
      ['regular expressions', /a+/gi, /a+/gi],
      [
        'objects with no prototype',
        Object.assign(Object.create(null), { a: 1 }),
        Object.assign(Object.create(null), { a: 1 }),
      ],
    ])('should be true for %s', (_name, a, b) => {
      expect(deepEqual(a, b)).toBe(true);
    });
  });

  describe('when the two values differ', () => {
    it.each([
      ['numbers', 1, 2],
      ['a number and its text', 1, '1'],
      ['null and an empty object', null, {}],
      ['null and undefined', null, undefined],
      ['an object with one more key', { a: 1 }, { a: 1, b: undefined }],
      ['a missing key and a key with the value undefined', { a: undefined }, { b: undefined }],
      ['a value at depth', { a: { b: [1, { c: 2 }] } }, { a: { b: [1, { c: 3 }] } }],
      ['arrays of another length', [1, 2], [1, 2, 3]],
      ['arrays in another order', [1, 2], [2, 1]],
      ['an array and an object with the same keys', [1], { 0: 1 }],
      ['dates', new Date(1), new Date(2)],
      ['regular expressions with other flags', /a/g, /a/i],
    ])('should be false for %s', (_name, a, b) => {
      expect(deepEqual(a, b)).toBe(false);
    });
  });
});
