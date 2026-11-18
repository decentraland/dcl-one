import { beforeEach, describe, expect, it } from 'vitest';

import { future, type IFuture } from '../future';

describe('future', () => {
  let pending: IFuture<number>;

  beforeEach(() => {
    pending = future<number>();
  });

  describe('when it is not settled', () => {
    it('should be a pending promise', () => {
      expect(pending).toBeInstanceOf(Promise);
      expect(pending.isPending).toBe(true);
    });
  });

  describe('when it is resolved', () => {
    beforeEach(() => {
      pending.resolve(7);
    });

    it('should give the value and not be pending', async () => {
      expect(pending.isPending).toBe(false);
      await expect(pending).resolves.toBe(7);
    });

    it('should keep the first value', async () => {
      pending.resolve(8);
      pending.reject(new Error('late'));

      await expect(pending).resolves.toBe(7);
    });
  });

  describe('when it is rejected', () => {
    it('should reject with the error and not be pending', async () => {
      pending.reject(new Error('failed'));

      expect(pending.isPending).toBe(false);
      await expect(pending).rejects.toThrow('failed');
    });
  });
});
