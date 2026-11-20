/**
 * Runs at most `max` tasks at once, first come first served. A finished task hands its slot straight to the next one
 * waiting, so a new caller can't slip in between; a waiting task whose signal aborted is dropped without starting.
 */
export function createLimiter(max: number) {
  let running = 0
  const waiting: Array<() => void> = []
  return async function limited<T>(task: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    if (running < max) running++
    else await new Promise<void>(resolve => waiting.push(resolve))
    try {
      signal?.throwIfAborted()
      return await task()
    } finally {
      const next = waiting.shift()
      if (next) next()
      else running--
    }
  }
}
