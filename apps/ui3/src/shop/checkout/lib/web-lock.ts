// A Web Lock held until the returned release, or until the page goes (the browser lets a closed or crashed page's locks
// go): the way to make one tab of this browser the only one running something, such as a credits + MANA sale of one item.

/** Takes `name` only if no page holds it: null when one does, and null too without Web Locks, where nothing proves it. */
export function webLock(locks: LockManager | null | undefined, name: string): Promise<(() => void) | null> {
  return new Promise((resolve) => {
    if (!locks) return resolve(null);
    locks
      .request(name, { ifAvailable: true }, (lock) => {
        if (!lock) {
          resolve(null);
          return undefined;
        }
        return new Promise<void>((release) => resolve(() => release()));
      })
      .catch(() => resolve(null));
  });
}

/** This page's Web Locks, or null where the browser has none. */
export function pageLocks(): LockManager | null {
  try {
    return typeof navigator !== "undefined" && navigator.locks ? navigator.locks : null;
  } catch {
    return null;
  }
}

/** A new id for one run of something held under a lock (a sale's or an approval's attempt): what its records are kept by. */
export function newAttemptId(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
