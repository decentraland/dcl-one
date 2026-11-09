// from: shop@94584a5c:app/src/lib/payments-stripe.ts
// The real (Stripe-backed) pack path: POST /shop-api/v1/pack-orders answers the hosted Checkout URL the
// browser goes to (Stripe returns to `/shop/credits?order=<id>`, or `&canceled=1`), and GET
// /shop-api/v1/pack-orders/{orderId} is polled until the verified webhook grants the credits. The
// buyer's IANA zone rides on the order request (coarse, identifies nobody). A 404 right after the return
// is read-replica lag, not a failure: the poll keeps waiting, and a paid order that never shows up in the
// window reads as 'pending' ("on the way"), never as an error.

import { isShopApiError } from "../api/client";
import type { ShopClient } from "../api/shop";
import type { CheckoutSession, OrderStatus } from "./payments";

function buyerTimezone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined;
  } catch {
    return undefined;
  }
}

export async function createPackCheckoutReal(packId: string, client: ShopClient): Promise<CheckoutSession> {
  const timezone = buyerTimezone();
  const { orderId, url } = await client.packs.order({ packId, ...(timezone ? { timezone } : {}), source: "website" });
  if (!orderId) throw new Error("Checkout did not return an order");
  return { orderId, ...(url ? { url } : {}), mock: false };
}

/** Statuses the poll keeps waiting on: money in without the grant, a return that beat the webhook, or an order not found yet. */
function stillWaiting(status: OrderStatus["status"]): boolean {
  return status === "processing" || status === "crediting" || status === "initiated" || status === "missing";
}

const delay = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const t = setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(t);
        reject(new DOMException("Aborted", "AbortError"));
      },
      { once: true },
    );
  });

/** The node's answer for an order it has no record of for this signer (credits/handlers/orders.rs, `order not found`). */
function isOrderNotFound(body: unknown): boolean {
  const b = body as { error?: unknown; message?: unknown } | null;
  return !!b && (b.error === "order not found" || b.message === "order not found");
}

async function fetchOrderStatus(orderId: string, client: ShopClient, signal?: AbortSignal): Promise<OrderStatus> {
  try {
    const answer = (await client.packs.orderStatus(orderId, { signal })) as OrderStatus | null;
    return answer && typeof answer.status === "string" ? answer : { status: "processing" };
  } catch (e) {
    // The node's own "order not found": no order by that id for this signer. Any other 404 (a gateway's route during a
    // deploy) says nothing about the order, so the poll keeps waiting on it as before.
    if (isShopApiError(e) && e.status === 404) return isOrderNotFound(e.body) ? { status: "missing" } : { status: "processing" };
    throw e;
  }
}

export async function pollCreditGrantReal(
  orderId: string,
  opts: { intervalMs?: number; timeoutMs?: number; missingGraceMs?: number; signal?: AbortSignal; now?: () => number } = {},
  client: ShopClient,
): Promise<OrderStatus> {
  const { intervalMs = 1500, timeoutMs = 60_000, missingGraceMs = 15_000, signal } = opts;
  const now = opts.now ?? Date.now;
  const started = now();
  const deadline = started + timeoutMs;
  let seen = false;
  for (;;) {
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    const status = await fetchOrderStatus(orderId, client, signal);
    if (status.status !== "missing") seen = true;
    if (!stillWaiting(status.status)) return status;
    // The node writes the order before its checkout opens, so a return normally finds it at once; the grace is a margin,
    // not a wait. Never found past it: no such order for this signer, and nothing is on its way.
    if (!seen && now() - started > missingGraceMs) return { status: "missing" };
    if (now() > deadline) {
      if (!seen) return { status: "missing" };
      // Still 'initiated': nobody paid, so nothing is on its way. Anything else is money in.
      if (status.status === "initiated") return status;
      return { status: "pending" };
    }
    await delay(intervalMs, signal);
  }
}
