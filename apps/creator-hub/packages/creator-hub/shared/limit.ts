export type Limit = <T>(task: () => PromiseLike<T> | T) => Promise<T>;

/**
 * Gives a function that runs at most `concurrency` tasks at one time. A task that comes while
 * all the places are taken waits for its turn, in the order of the calls.
 */
export function createLimit(concurrency: number): Limit {
  if (!Number.isInteger(concurrency) || concurrency < 1) {
    throw new TypeError(`Expected a concurrency of 1 or more, got ${concurrency}`);
  }
  const waiting: (() => void)[] = [];
  let active = 0;

  // A task that ends gives its place to the next waiting task. The count goes down only when
  // no task waits, so a call that comes between the two cannot take the place.
  const release = () => {
    const next = waiting.shift();
    if (next) next();
    else active--;
  };

  return async task => {
    if (active < concurrency) active++;
    else await new Promise<void>(resolve => waiting.push(resolve));
    try {
      return await task();
    } finally {
      release();
    }
  };
}
