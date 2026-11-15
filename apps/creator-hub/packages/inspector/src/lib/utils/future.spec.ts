import { beforeEach, describe, expect, it } from 'vitest';

import type { IFuture } from './future';
import { future } from './future';

describe('future', () => {
  let pending: IFuture<number>;

  beforeEach(() => {
    pending = future<number>();
  });

  describe('when it is resolved', () => {
    beforeEach(() => {
      pending.resolve(7);
    });

    it('should give the value', async () => {
      expect(pending).toBeInstanceOf(Promise);
      await expect(pending).resolves.toBe(7);
    });

    it('should keep the first value', async () => {
      pending.resolve(8);
      pending.reject(new Error('late'));

      await expect(pending).resolves.toBe(7);
    });
  });

  describe('when it is rejected', () => {
    it('should give the error', async () => {
      pending.reject(new Error('stopped'));

      await expect(pending).rejects.toThrow('stopped');
    });
  });

  describe('when a function that waits for it starts before it is settled', () => {
    it('should give that function the value', async () => {
      const waiting = pending.then(value => value + 1);

      pending.resolve(1);

      await expect(waiting).resolves.toBe(2);
    });
  });
});
