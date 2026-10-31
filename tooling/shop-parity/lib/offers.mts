// dcl-one's `POST /shop-api/v1/purchase-offers` has no upstream: the node derives it from its own
// records. While recording, it is answered from the recorded listing row the way
// services/catalyst/shop/src/mana/quote.rs answers a signed-out request for a primary trade
// (policy::present: credits only, default credits, reasons creditsFirst + signInForCredits, no
// guidance; quotes for the allowed methods only; settlement call accept / acceptWithCoupon).
// Anything else (a resale, LAND, a signed session) is left unanswered so the page shows it.
import type { Answer, Recording } from "./recording.mts";

type Line = { asset?: { chainId?: number; contract?: string; token?: { kind?: string; itemId?: string; tokenId?: string } }; listing?: { tradeId?: string }; quantity?: number };
type Row = Record<string, unknown>;

function rowsOf(recording: Recording): Row[] {
  const rows: Row[] = [];
  for (const e of recording.entries.values()) {
    if (e.service !== "marketplace" || !/json/.test(e.contentType) || !/catalog/.test(e.path)) continue;
    try {
      const body = JSON.parse(recording.body(e).toString("utf8")) as { data?: unknown };
      if (Array.isArray(body.data)) rows.push(...(body.data as Row[]));
    } catch {
      // not a catalogue page
    }
  }
  return rows;
}

export function offerSynthesizer(recording: Recording) {
  return async (req: { request: unknown }): Promise<Answer | null> => {
    const lines = ((req.request as { lines?: Line[] } | undefined)?.lines ?? []) as Line[];
    const rows = rowsOf(recording);
    const now = Date.now();
    const offers = lines.map((line, index) => {
      const a = line.asset;
      if (!a?.contract || a.token?.kind !== "item") return { index, error: "Bad Request", message: "not a primary item line" };
      const row = rows.find(
        (r) =>
          String(r.contractAddress).toLowerCase() === a.contract!.toLowerCase() &&
          String(r.itemId) === a.token!.itemId &&
          r.listingType === "primary" &&
          (!line.listing?.tradeId || r.tradeId === line.listing.tradeId),
      );
      if (!row || typeof row.priceCredits !== "number") return { index, error: "Bad Request", message: "listing not in the recording" };
      const chainId = Number(row.chainId ?? a.chainId);
      const contract = String(row.tradeContractAddress ?? row.contractAddress).toLowerCase();
      const call = row.coupon ? { kind: "acceptWithCoupon" } : { kind: "accept" };
      return {
        asset: a,
        kind: row.category === "emote" ? "emote" : "wearable",
        sale: "primaryMint",
        bundle: false,
        eligibility: { allowed: ["credits"], default: "credits", reasons: ["creditsFirst", "signInForCredits"], guidance: null },
        quotes: [{ method: "credits", quoted: row.priceCredits * 10 }],
        unavailable: [],
        settlement: [{ method: "credits", key: { method: "credits", chainId, contract, call } }],
        available: typeof row.available === "number" ? row.available : row.available === null || row.available === undefined ? null : Number(row.available),
        quotedAt: now,
      };
    });
    return { status: 200, contentType: "application/json", body: Buffer.from(JSON.stringify({ quotedAt: now, offers })) };
  };
}
