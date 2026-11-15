import { describe, expect, it } from 'vitest';

import { flatten } from './flatten';

describe('flatten', () => {
  describe('when the object has nested objects', () => {
    it('should give one key for each value, with the path joined by dots', () => {
      const flat = flatten({ modal: { title: 'Title', actions: { ok: 'OK' } }, name: 'Name' });

      expect(flat).toEqual({ 'modal.title': 'Title', 'modal.actions.ok': 'OK', name: 'Name' });
    });
  });

  describe('when the object has an array', () => {
    it('should give one key for each index', () => {
      expect(flatten({ steps: ['one', { text: 'two' }] })).toEqual({
        'steps.0': 'one',
        'steps.1.text': 'two',
      });
    });
  });

  describe('when a value is an empty object, an empty array or not an object', () => {
    it('should keep the value', () => {
      const date = new Date(0);

      expect(flatten({ a: {}, b: { c: [] }, d: null, e: 0, f: date })).toEqual({
        a: {},
        'b.c': [],
        d: null,
        e: 0,
        f: date,
      });
    });
  });
});
