// from: shop@94584a5c:app/src/lib/purchase-report.ts
// Reports the transaction a group's credits went out in (POST /shop-api/v1/credit-authorizations/submitted),
// so a purchase that was attempted and reverted can be told apart from a reservation nobody spent. FIRE
// AND FORGET: bookkeeping must never fail a purchase already on its way. Every giving-up path is named
// (to `onGaveUp`, console by default) rather than swallowed, since a silent zero looks like success.

import { shopClient, type ShopClient } from "../../api/shop";
import { reportIntentSubmission } from "../../credits/credits";

export type ReportOptions = { txHash: string; salts: string[]; client?: ShopClient; onGaveUp?: (reason: string, detail?: unknown) => void };

const warn = (reason: string, detail?: unknown) => console.warn(`[shop/report] submitted-tx report gave up: ${reason}`, detail ?? "");

/** Returns the pending report, for tests; callers never await it. */
export function reportSubmittedTx(info: ReportOptions): Promise<void> {
  const gaveUp = info.onGaveUp ?? warn;
  const salts = [...new Set(info.salts)];
  if (salts.length === 0 || !info.txHash) {
    gaveUp("nothing to report", { salts: salts.length, hasTxHash: !!info.txHash });
    return Promise.resolve();
  }
  let client: ShopClient;
  try {
    client = info.client ?? shopClient();
  } catch (e) {
    gaveUp("no client", e);
    return Promise.resolve();
  }
  return reportIntentSubmission(salts, info.txHash, client).then(
    (recorded) => {
      if (recorded === 0) gaveUp("server recorded 0 rows", { salts: salts.length });
    },
    (e: unknown) => gaveUp("request failed", e),
  );
}
