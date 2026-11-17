// from: shop@94584a5c:app/src/lib/spend-guard.ts
// "May these credits already be consumed on-chain?": the one question that decides whether a failed
// purchase may release its reservation. Kept PER CREDIT and PER TRANSACTION HASH, not as a flag: a credit
// can back several attempts, and a later attempt's revert says nothing about an earlier broadcast whose
// outcome was never observed. A credit is releasable only when every hash broadcast for it reverted and
// no submit is unaccounted for. Releasing a consumed credit corrupts the balance; failing to release an
// unconsumed one only strands it until its TTL. When in doubt, do not release.

export type SpendGuard = {
  submitStarted(creditId: string): void;
  submitFinished(creditId: string): void;
  broadcast(creditId: string, txHash: string): void;
  reverted(txHash: string): void;
  /** A submit failed without a usable answer (no hash to key on): permanently unclearable. */
  unobservable(creditId: string): void;
  isInFlight(creditId: string): boolean;
  /** True means the credits MAY be consumed, so they must NOT be released. */
  mayBeConsumed(creditId: string): boolean;
};

export function createSpendGuard(): SpendGuard {
  const hashesByCredit = new Map<string, Set<string>>();
  const revertedHashes = new Set<string>();
  const unobservable = new Set<string>();
  const inFlight = new Set<string>();
  return {
    submitStarted: (id) => void inFlight.add(id),
    submitFinished: (id) => void inFlight.delete(id),
    broadcast(id, txHash) {
      const set = hashesByCredit.get(id) ?? new Set<string>();
      set.add(txHash.toLowerCase());
      hashesByCredit.set(id, set);
    },
    reverted: (txHash) => void revertedHashes.add(txHash.toLowerCase()),
    unobservable: (id) => void unobservable.add(id),
    isInFlight: (id) => inFlight.has(id),
    mayBeConsumed(id) {
      if (unobservable.has(id)) return true;
      const hashes = hashesByCredit.get(id);
      if (!hashes) return false;
      for (const hash of hashes) if (!revertedHashes.has(hash)) return true;
      return false;
    },
  };
}
