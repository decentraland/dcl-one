// from: shop@94584a5c:app/src/lib/purchases.ts
// Purchase history folded back into orders: one card per checkout transaction. Lines sharing a
// settlement hash are one order; lines with no hash (older rows, still pending) fold by time proximity
// and equal status, since one transaction settles all-or-nothing. An order is PENDING while any line is,
// SETTLED if any line settled, EXPIRED otherwise; EXPIRED rows only reach here when they were actually
// submitted (see activity.ts), so the pill says FAILED for them.

import type { PurchaseRecord } from "../credits/credits";

export type PurchaseOrder = {
  /** The shared settlement hash when present, else the earliest line's id. */
  id: string;
  createdAt: number;
  status: "PENDING" | "SETTLED" | "EXPIRED";
  totalCredits: number;
  lines: PurchaseRecord[];
};

const GROUP_WINDOW_MS = 15_000;

function sameOrder(a: PurchaseRecord, b: PurchaseRecord): boolean {
  if (a.txHash && b.txHash) return a.txHash.toLowerCase() === b.txHash.toLowerCase();
  if (a.txHash || b.txHash) return false;
  return a.status === b.status && Math.abs(a.createdAt - b.createdAt) <= GROUP_WINDOW_MS;
}

function orderStatus(lines: PurchaseRecord[]): PurchaseOrder["status"] {
  if (lines.some((l) => l.status === "PENDING")) return "PENDING";
  if (lines.some((l) => l.status === "SETTLED")) return "SETTLED";
  return "EXPIRED";
}

/** Orders, newest first (and newest line first within one). */
export function groupPurchases(records: PurchaseRecord[]): PurchaseOrder[] {
  const sorted = [...records].sort((a, b) => b.createdAt - a.createdAt);
  const groups: PurchaseRecord[][] = [];
  for (const record of sorted) {
    const current = groups[groups.length - 1];
    const last = current?.[current.length - 1];
    if (current && last && sameOrder(last, record)) current.push(record);
    else groups.push([record]);
  }
  return groups.map((lines) => ({
    id: lines.find((l) => l.txHash)?.txHash ?? lines[lines.length - 1]!.id,
    createdAt: Math.max(...lines.map((l) => l.createdAt)),
    status: orderStatus(lines),
    totalCredits: lines.reduce((sum, l) => sum + l.credits, 0),
    lines,
  }));
}

/** An order's status in the Activity pill's vocabulary. */
export function purchaseOrderPill(status: PurchaseOrder["status"]): "SETTLED" | "PENDING" | "FAILED" {
  if (status === "SETTLED") return "SETTLED";
  if (status === "EXPIRED") return "FAILED";
  return "PENDING";
}

export type OrderLineItem = {
  key: string;
  tradeId: string | null;
  contractAddress: string | null;
  itemId: string | null;
  registeredName: string | null;
  quantity: number;
  credits: number;
};

function foldKey(line: PurchaseRecord): string | null {
  if (line.tradeId) return `trade:${line.tradeId}`;
  if (line.contractAddress && line.itemId) return `item:${line.contractAddress.toLowerCase()}:${line.itemId}`;
  return null;
}

/** One rendered row per thing bought: the same item bought N times is one line with a quantity. */
export function foldOrderLines(lines: PurchaseRecord[]): OrderLineItem[] {
  const out: OrderLineItem[] = [];
  const byKey = new Map<string, OrderLineItem>();
  for (const line of lines) {
    const key = foldKey(line);
    const existing = key ? byKey.get(key) : undefined;
    if (existing) {
      existing.quantity += 1;
      existing.credits += line.credits;
      continue;
    }
    const item: OrderLineItem = {
      key: key ?? line.id,
      tradeId: line.tradeId,
      contractAddress: line.contractAddress,
      itemId: line.itemId,
      registeredName: line.registeredName,
      quantity: 1,
      credits: line.credits,
    };
    if (key) byKey.set(key, item);
    out.push(item);
  }
  return out;
}
