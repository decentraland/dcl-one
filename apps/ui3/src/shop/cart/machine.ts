// The per-group checkout state machine. Different chain transactions cannot promise an atomic basket
// (SHOP.md), so each confirmation group advances alone:
//
//   idle -> authorizing -> submitted -> pending -> confirmed
//                 \             \          \
//                  failed/expired  failed (a rejection before broadcast, or a confirmed revert)
//
// `submitted` is entered the moment a transaction is handed to a wallet or the relay, before any hash
// is known: from then on the group may have been broadcast, so it is only ever watched, never re-run.
// A resume re-runs idle, failed and expired groups (and authorizing ones, releasing their credits
// first), watches submitted and pending ones, and leaves confirmed ones alone.

import type { GroupFailure, GroupProgress, GroupStatus, PurchaseState } from "../model/purchase";
import type { CartGroup } from "./partition";

export type GroupEvent =
  | { type: "authorize" }
  | { type: "authorized"; creditIds: string[] }
  | { type: "submit" }
  | { type: "hash"; txHash: string }
  | { type: "observed" }
  | { type: "confirmed"; txHash?: string }
  | { type: "failed"; failure: GroupFailure }
  | { type: "expired" };

export class InvalidTransition extends Error {
  override name = "InvalidTransition";
  constructor(
    readonly from: GroupStatus,
    readonly event: GroupEvent["type"],
  ) {
    super(`a ${from} group cannot take ${event}`);
  }
}

const RESTARTABLE: readonly GroupStatus[] = ["idle", "failed", "expired"];

function failureAllowed(group: GroupProgress, failure: GroupFailure): boolean {
  if (group.status === "authorizing") return true;
  if (group.status === "submitted" || group.status === "pending") {
    if (failure.kind === "reverted") return group.txHash !== null;
    return failure.kind === "rejected" && group.txHash === null && group.status === "submitted";
  }
  return false;
}

export function canTransition(group: GroupProgress, event: GroupEvent): boolean {
  switch (event.type) {
    case "authorize":
      return RESTARTABLE.includes(group.status);
    case "authorized":
    case "submit":
    case "expired":
      return group.status === "authorizing";
    case "hash":
      return group.status === "submitted" || group.status === "pending";
    case "observed":
      return group.status === "submitted";
    case "confirmed":
      return group.status === "submitted" || group.status === "pending";
    case "failed":
      return failureAllowed(group, event.failure);
  }
}

export function transition(group: GroupProgress, event: GroupEvent, now: number = Date.now()): GroupProgress {
  if (!canTransition(group, event)) throw new InvalidTransition(group.status, event.type);
  const base = { ...group, updatedAt: now };
  switch (event.type) {
    case "authorize":
      return { ...base, status: "authorizing", failure: null, txHash: null, creditIds: [], attempts: group.attempts + 1 };
    case "authorized":
      return { ...base, creditIds: [...event.creditIds] };
    case "submit":
      return { ...base, status: "submitted" };
    case "hash":
      return { ...base, txHash: event.txHash };
    case "observed":
      return { ...base, status: "pending" };
    case "confirmed":
      return { ...base, status: "confirmed", txHash: event.txHash ?? group.txHash, failure: null };
    case "failed":
      return { ...base, status: "failed", failure: event.failure };
    case "expired":
      return { ...base, status: "expired" };
  }
}

export function newCheckout(account: string, groups: CartGroup[], now: number = Date.now(), id?: string): PurchaseState {
  return {
    id: id ?? `${account.toLowerCase()}:${now}`,
    account: account.toLowerCase(),
    createdAt: now,
    groups: groups.map((g) => ({
      keyId: g.keyId,
      key: g.key,
      lineIds: [...g.lineIds],
      status: "idle",
      creditIds: [],
      txHash: null,
      failure: null,
      updatedAt: now,
      attempts: 0,
    })),
  };
}

export function applyEvent(state: PurchaseState, keyId: string, event: GroupEvent, now: number = Date.now()): PurchaseState {
  const index = state.groups.findIndex((g) => g.keyId === keyId);
  if (index < 0) throw new Error(`no group ${keyId} in checkout ${state.id}`);
  const groups = state.groups.slice();
  groups[index] = transition(state.groups[index]!, event, now);
  return { ...state, groups };
}

export type ResumePlan = {
  /** Groups to start (again). */
  run: GroupProgress[];
  /** Credit salts to cancel before re-running: authorizations nothing was handed off for. */
  release: string[];
  /** Groups that may have been broadcast: follow their transaction, never re-run. */
  watch: GroupProgress[];
  done: GroupProgress[];
};

export function resumePlan(state: PurchaseState): ResumePlan {
  const plan: ResumePlan = { run: [], release: [], watch: [], done: [] };
  for (const group of state.groups) {
    if (group.status === "confirmed") plan.done.push(group);
    else if (group.status === "submitted" || group.status === "pending") plan.watch.push(group);
    else {
      plan.run.push(group);
      if (group.status === "authorizing") plan.release.push(...group.creditIds);
    }
  }
  return plan;
}

/** Puts every group the plan re-runs back to idle, so `authorize` can start it. */
export function prepareResume(state: PurchaseState, now: number = Date.now()): PurchaseState {
  return {
    ...state,
    groups: state.groups.map((g) =>
      g.status === "authorizing" ? { ...g, status: "idle" as const, creditIds: [], updatedAt: now } : g,
    ),
  };
}

export type CheckoutSummary = Record<GroupStatus, number> & { complete: boolean; settled: number; total: number };

export function checkoutSummary(state: PurchaseState): CheckoutSummary {
  const counts: Record<GroupStatus, number> = {
    idle: 0,
    authorizing: 0,
    submitted: 0,
    pending: 0,
    confirmed: 0,
    failed: 0,
    expired: 0,
  };
  for (const g of state.groups) counts[g.status] += 1;
  return {
    ...counts,
    complete: counts.confirmed === state.groups.length,
    settled: counts.confirmed,
    total: state.groups.length,
  };
}
