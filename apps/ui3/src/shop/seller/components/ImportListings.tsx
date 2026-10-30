// from: shop@94584a5c:app/src/components/ImportListings/ImportListings.tsx, app/src/components/MigrateModal, app/src/lib/import.ts
// "Bring your listings into the Shop": a creator's classic MANA listings of their OWN collections become
// credit-buyable primary listings at a price they confirm (suggested from the server's credit quote of the
// old MANA price). Owned tokens are resales and are not offered: they keep their MANA listing (SHOP.md,
// the listing-import restriction). Items import one at a time, each telling which wallet prompt or wait
// it is in; a failure after the old listing came down says the item is now unlisted.

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import { CENTS_PER_CREDIT, formatUsd, parseManaWei, usdCents } from "../../model/money";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { LoadFailure, Loading, ManaAmount, useShopProvider, useWalletBlock, writeFailureText } from "../../market/components/kit";
import { useImportable } from "../hooks";
import { countConfirmations, importListing, RelistFailedError, type ImportItem, type ImportPhase } from "../lib/import";

const PHASE_TEXT: Record<ImportPhase["step"], string> = {
  cancelling: "Taking the old listing down: confirm in your wallet",
  "confirming-cancel": "Waiting for the old listing to be cancelled on chain",
  authorising: "Letting the Shop mint this collection (once)",
  signing: "Sign the new listing in your wallet",
  publishing: "Publishing",
  indexing: "Waiting for the old listing to clear",
};

type Outcome = { state: "done" } | { state: "failed"; message: string; unlisted: boolean } | { state: "working"; phase: string };

export function ImportListingsView({ creations, owned }: { creations: ImportItem[]; owned: ImportItem[] }) {
  const qc = useQueryClient();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const block = useWalletBlock("Moving listings into the Shop");
  const [selected, setSelected] = useState<Record<string, boolean>>(() => Object.fromEntries(creations.map((c) => [c.oldTradeId, c.suggestedCredits !== null])));
  const [prices, setPrices] = useState<Record<string, string>>(() => Object.fromEntries(creations.map((c) => [c.oldTradeId, c.suggestedCredits ? String(c.suggestedCredits) : ""])));
  const [outcomes, setOutcomes] = useState<Record<string, Outcome>>({});
  const [running, setRunning] = useState(false);
  const [confirmations, setConfirmations] = useState<{ total: number } | null>(null);

  const chosen = useMemo(() => creations.filter((c) => selected[c.oldTradeId] && outcomes[c.oldTradeId]?.state !== "done"), [creations, selected, outcomes]);
  const priceOf = (c: ImportItem) => {
    const v = Number(prices[c.oldTradeId]);
    return /^\d+$/.test(prices[c.oldTradeId] ?? "") && v >= 1 ? v : null;
  };
  const invalid = chosen.some((c) => priceOf(c) === null);
  const totalCents = chosen.reduce((n, c) => n + (priceOf(c) ?? 0) * CENTS_PER_CREDIT, 0);

  useEffect(() => {
    if (!account || chosen.length === 0) return;
    let live = true;
    void countConfirmations(chosen, account).then((n) => live && setConfirmations(n));
    return () => {
      live = false;
    };
  }, [account, chosen]);

  async function run() {
    if (!provider) return;
    setRunning(true);
    for (const item of chosen) {
      const price = priceOf(item)!;
      setOutcomes((o) => ({ ...o, [item.oldTradeId]: { state: "working", phase: "Starting" } }));
      try {
        await importListing(item, price, {
          provider,
          cancelOld: outcomes[item.oldTradeId]?.state === "failed" && (outcomes[item.oldTradeId] as { unlisted: boolean }).unlisted ? false : true,
          onPhase: (p) => setOutcomes((o) => ({ ...o, [item.oldTradeId]: { state: "working", phase: p.step === "indexing" ? `${PHASE_TEXT.indexing} (attempt ${p.attempt} of ${p.of})` : PHASE_TEXT[p.step] } })),
        });
        setOutcomes((o) => ({ ...o, [item.oldTradeId]: { state: "done" } }));
      } catch (e) {
        setOutcomes((o) => ({ ...o, [item.oldTradeId]: { state: "failed", message: e instanceof RelistFailedError ? e.message : writeFailureText(e), unlisted: e instanceof RelistFailedError } }));
      }
    }
    setRunning(false);
    await qc.invalidateQueries({ queryKey: ["importable"] });
    await qc.invalidateQueries({ queryKey: ["store-sale-state"] });
  }

  if (creations.length === 0 && owned.length === 0)
    return (
      <EmptyState variant="inline" title="You are all set!" subtitle="Nothing of yours is listed in MANA only." actions={[{ label: "My items", href: shopPath("myItems") }]} data-testid="shop-import-empty" />
    );

  return (
    <div className="mk-section" data-testid="shop-import">
      {creations.length > 0 ? (
        <>
          <p>
            Your creations can now be priced in Credits. We suggest prices from today's MANA rate; adjust them before listing. Each item replaces its old MANA listing.
          </p>
          {block}
          <ul className="mk-list">
            {creations.map((c) => {
              const outcome = outcomes[c.oldTradeId];
              const wei = parseManaWei(c.manaWei);
              return (
                <li key={c.oldTradeId} className="mk-row" data-testid="shop-import-row">
                  <label className="mk-row__main mk-inline-check">
                    <input
                      type="checkbox"
                      checked={!!selected[c.oldTradeId]}
                      disabled={running || outcome?.state === "done"}
                      onChange={(e) => setSelected((s) => ({ ...s, [c.oldTradeId]: e.target.checked }))}
                      aria-label={`Include ${c.name}`}
                    />
                    <span>{c.name}</span>
                    <span className="mk-row__sub">({wei === null ? "?" : <ManaAmount wei={wei} />})</span>
                  </label>
                  <label className="mk-field">
                    <span className="u-sr-only">Price in Credits for {c.name}</span>
                    <input inputMode="numeric" value={prices[c.oldTradeId] ?? ""} onChange={(e) => setPrices((p) => ({ ...p, [c.oldTradeId]: e.target.value }))} disabled={running || outcome?.state === "done"} placeholder="Credits" />
                  </label>
                  <span className="mk-row__sub" role={outcome?.state === "failed" ? "alert" : "status"}>
                    {outcome?.state === "done" ? "Listed in the Shop" : outcome?.state === "working" ? outcome.phase : outcome?.state === "failed" ? outcome.message : c.suggestedCredits === null ? "Set a price" : ""}
                  </span>
                </li>
              );
            })}
          </ul>
          <p className="mk-hint">
            {chosen.length} {chosen.length === 1 ? "item" : "items"} selected · {formatUsd(usdCents(totalCents))}
            {confirmations ? ` · ${confirmations.total} wallet confirmations` : ""}
          </p>
          <div className="mk-actions">
            <Button disabled={!provider || !!block || running || chosen.length === 0 || invalid} onClick={() => void run()}>
              {running ? "Listing…" : `List ${chosen.length} ${chosen.length === 1 ? "item" : "items"}`}
            </Button>
          </div>
        </>
      ) : null}
      {owned.length > 0 ? (
        <div className="mk-panel" data-testid="shop-import-owned">
          <h3 className="mk-panel__title">Resales stay in MANA</h3>
          <p className="mk-hint">These are tokens you own, not your creations. Their listings stay as they are and buyers pay them in MANA.</p>
          <ul className="mk-list">
            {owned.map((o) => {
              const wei = parseManaWei(o.manaWei);
              return (
                <li key={o.oldTradeId} className="mk-row">
                  <span className="mk-row__main">{o.name}</span>
                  <span>{wei === null ? "—" : <ManaAmount wei={wei} />}</span>
                  {o.tokenId ? (
                    <Link className="mk-link" to={shopPath("tokenSell", { contractAddress: o.contractAddress, tokenId: o.tokenId })}>
                      Change price
                    </Link>
                  ) : (
                    <span />
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

/** The import tool as a panel: My Store renders it, and so can the Activity page's listings section. */
export default function ImportListings() {
  const { account } = useShopWallet();
  const query = useImportable(account);
  if (!account) return null;
  if (query.isPending) return <Loading label="Looking for listings to move…" />;
  if (query.isError) return <LoadFailure error={query.error} what="Your listings" onRetry={() => void query.refetch()} />;
  return <ImportListingsView creations={query.data.creations} owned={query.data.owned} />;
}
