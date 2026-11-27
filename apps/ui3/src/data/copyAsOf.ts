import { useSyncExternalStore } from "react";

import { copiesVersion, copyAsOf, subscribeCopies, type CopyDomain, type CopyStamp } from "@web-core/freshness/copyAsOf";

export { copyAsOf, noteCopy, olderCopy, readCopy, resetCopies, type CopyDomain, type CopyStamp } from "@web-core/freshness/copyAsOf";

export function useCopyAsOf(domains: readonly CopyDomain[]): CopyStamp | null {
  const live = useSyncExternalStore(subscribeCopies, copiesVersion, () => -1) === -1;
  return live ? null : copyAsOf(domains);
}
