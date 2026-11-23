const STORAGE_PREFIX = 'review-modal-seen:'
// Kept for the page's lifetime too: with storage blocked, a closed modal must not reopen on the next render.
const seenThisVisit = new Set<string>()

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    return null
  }
}

/** Whether the creator already saw the modal for this verdict or hand-off of the collection. */
export function wasReviewModalSeen(collectionId: string, key: string): boolean {
  if (seenThisVisit.has(`${collectionId}:${key}`)) return true
  try {
    return storage()?.getItem(STORAGE_PREFIX + collectionId) === key
  } catch {
    return false
  }
}

/** Remembers the modal was shown, so reopening the page shows the next verdict only. Best effort. */
export function markReviewModalSeen(collectionId: string, key: string): void {
  seenThisVisit.add(`${collectionId}:${key}`)
  try {
    storage()?.setItem(STORAGE_PREFIX + collectionId, key)
  } catch {
    // Private windows may refuse writes: the modal then shows again on the next visit only.
  }
}
