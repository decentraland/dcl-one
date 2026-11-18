import { describe, expect, it } from 'vitest';
import { Queue } from '../scripts/utils/queue.ts';

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const tick = () => new Promise(resolve => setImmediate(resolve));

describe('Queue', () => {
  describe('when more tasks are added than the limit', () => {
    it('should run at most that many at the same time and run them all', async () => {
      const queue = new Queue(2);
      const gates = [deferred(), deferred(), deferred(), deferred(), deferred()];
      let running = 0;
      let peak = 0;
      let done = 0;
      for (const gate of gates) {
        queue.add(async () => {
          peak = Math.max(peak, ++running);
          await gate.promise;
          running--;
          done++;
        });
      }
      const idle = queue.onIdle();

      await tick();
      expect(running).toBe(2);
      for (const gate of gates) {
        gate.resolve();
        await tick();
      }
      await idle;

      expect(peak).toBe(2);
      expect(done).toBe(5);
    });
  });

  describe('when the queue has no tasks', () => {
    it('should be idle', async () => {
      await expect(new Queue(1).onIdle()).resolves.toBeUndefined();
    });
  });

  describe('when a task fails', () => {
    it('should start no more tasks and reject with the error of the task', async () => {
      const queue = new Queue(1);
      const error = new Error('upload failed');
      const started: string[] = [];
      queue.add(async () => {
        started.push('first');
        throw error;
      });
      queue.add(async () => {
        started.push('second');
      });

      await expect(queue.onIdle()).rejects.toBe(error);
      queue.add(async () => {
        started.push('third');
      });
      await tick();

      expect(started).toEqual(['first']);
      await expect(queue.onIdle()).rejects.toBe(error);
    });
  });
});
