import { createLimiter } from '../limiter'

// Caps how many validations download and parse at once: a 50-item collection must not fetch 50 models in parallel.
export const MAX_CONCURRENT = 2

/** Runs `task` once a slot frees up. A run whose signal aborted while it waited is dropped without starting. */
export const limited = createLimiter(MAX_CONCURRENT)
