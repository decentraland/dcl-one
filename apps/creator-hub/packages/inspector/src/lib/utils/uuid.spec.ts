import { afterEach, describe, expect, it, vi } from 'vitest';

import { randomUuid } from './uuid';

const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe('randomUuid', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe('when the page has crypto.randomUUID', () => {
    it('should give a version 4 UUID that changes on each call', () => {
      const first = randomUuid();

      expect(first).toMatch(UUID_V4);
      expect(randomUuid()).not.toBe(first);
    });
  });

  describe('when the page does not have crypto.randomUUID', () => {
    it('should make a version 4 UUID from random bytes', () => {
      const getRandomValues = vi.fn((bytes: Uint8Array) => bytes.fill(0xff));
      vi.stubGlobal('crypto', { getRandomValues });

      expect(randomUuid()).toBe('ffffffff-ffff-4fff-bfff-ffffffffffff');
      expect(getRandomValues).toHaveBeenCalledTimes(1);
    });
  });
});
