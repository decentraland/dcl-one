// A module loaded on first use and kept: a load that fails (a network blip, a chunk a deploy rotated away)
// is forgotten, so the next call loads it again instead of answering the same failure until a reload.

export function loadOnce<T>(load: () => Promise<T>): () => Promise<T> {
  let loaded: Promise<T> | null = null
  return () =>
    (loaded ??= load().catch((e: unknown) => {
      loaded = null
      throw e
    }))
}
