import { beforeEach, describe, expect, it } from 'vitest';

import { future, type IFuture } from '../future';
import { createLimit, type Limit } from '../limit';

describe('createLimit', () => {
  let limit: Limit;
  let gates: IFuture<string>[];
  let started: number[];

  function run(index: number): Promise<string> {
    return limit(() => {
      started.push(index);
      return gates[index];
    });
  }

  beforeEach(() => {
    limit = createLimit(2);
    gates = [future<string>(), future<string>(), future<string>(), future<string>()];
    started = [];
  });

  describe('when more tasks come than the concurrency', () => {
    let results: Promise<string>[];

    beforeEach(() => {
      results = gates.map((_, index) => run(index));
    });

    it('should start only as many tasks as the concurrency', () => {
      expect(started).toEqual([0, 1]);
    });

    it('should start the next task in the order of the calls when a task ends', async () => {
      gates[1].resolve('b');
      await results[1];

      expect(started).toEqual([0, 1, 2]);
    });

    it('should give each caller the result of its own task', async () => {
      gates.forEach((gate, index) => gate.resolve(`result ${index}`));

      await expect(Promise.all(results)).resolves.toEqual([
        'result 0',
        'result 1',
        'result 2',
        'result 3',
      ]);
    });

    it('should not let a new call take the place that a waiting task gets', async () => {
      gates[0].resolve('a');
      await results[0];
      void run(3);
      await Promise.resolve();

      expect(started).toEqual([0, 1, 2]);
    });
  });

  describe('when a task fails', () => {
    it('should reject its caller and start the next task', async () => {
      const results = gates.map((_, index) => run(index));
      gates[0].reject(new Error('failed'));

      await expect(results[0]).rejects.toThrow('failed');
      expect(started).toEqual([0, 1, 2]);
    });

    it('should reject the caller of a task that throws before it gives a promise', async () => {
      const result = limit(() => {
        throw new Error('thrown');
      });

      await expect(result).rejects.toThrow('thrown');
    });
  });

  describe('when the task gives a value that is not a promise', () => {
    it('should resolve with the value', async () => {
      await expect(limit(() => 7)).resolves.toBe(7);
    });
  });

  describe('when the concurrency is not a positive integer', () => {
    it('should throw', () => {
      expect(() => createLimit(0)).toThrow(TypeError);
      expect(() => createLimit(1.5)).toThrow(TypeError);
    });
  });
});
