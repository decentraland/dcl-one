// from: marketplace@f3956826:webapp/src/modules/bid/actions.ts (ARCHIVE_BID, UNARCHIVE_BID), webapp/src/modules/ui/asset/bid/reducer.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// Hiding a received bid is local to the owner's browser, as upstream: the bid stays open on chain and to
// every other reader. Scoped per account so a shared browser never mixes two owners' archives.

import { useSyncExternalStore } from "react";

const KEY = "dcl-shop-archived-bids:v1";

type Archive = Record<string, string[]>;

const listeners = new Set<() => void>();
let cache: Archive | null = null;

function load(): Archive {
  if (cache) return cache;
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}") as unknown;
    cache = parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Archive) : {};
  } catch {
    cache = {};
  }
  return cache;
}

function save(next: Archive): void {
  cache = next;
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // The archive still holds for this session.
  }
  for (const l of listeners) l();
}

export function archivedBidIds(account: string | null): readonly string[] {
  return account ? (load()[account.toLowerCase()] ?? []) : [];
}

export function archiveBid(account: string, bidId: string): void {
  const key = account.toLowerCase();
  const current = load()[key] ?? [];
  if (!current.includes(bidId)) save({ ...load(), [key]: [...current, bidId] });
}

export function unarchiveBid(account: string, bidId: string): void {
  const key = account.toLowerCase();
  save({ ...load(), [key]: (load()[key] ?? []).filter((id) => id !== bidId) });
}

export function resetArchivedBidsForTests(): void {
  cache = null;
}

const EMPTY: readonly string[] = [];

export function useArchivedBids(account: string | null): readonly string[] {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => (account ? (load()[account.toLowerCase()] ?? EMPTY) : EMPTY),
    () => EMPTY,
  );
}
