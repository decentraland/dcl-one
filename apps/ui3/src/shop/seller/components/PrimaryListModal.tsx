// from: shop@94584a5c:app/src/components/PrimaryListModal/PrimaryListModal.tsx, app/src/lib/trades.ts (createPrimaryUsdPeggedListing,
//       ensureMinter), app/src/lib/import.ts (postListingWithRetry)
// A creator puts one of their published items on sale in the Shop: a PRIMARY listing, the one kind a
// credit can pay for. The price is whole credits (one credit is ten US cents), the listing mints up to the
// item's remaining supply, and the first listing from a collection first lets the marketplace mint it (a
// one-time, gasless approval). The creator is paid in MANA.

import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router";

import Button from "../../../atoms/Button";
import { CHAIN, type ChainId } from "../../model/chain";
import { CENTS_PER_CREDIT, formatUsd, usdCents } from "../../model/money";
import { shopPath } from "../../routes";
import { useShopProvider, useWalletBlock, useWrite, WriteStatus } from "../../market/components/kit";
import { createPrimaryUsdPeggedListing, ensureMinter } from "../../market/lib/trades";
import type { PublishableItem } from "../lib/builder";
import { postListingWithRetry } from "../lib/import";

const SIX_MONTHS_MS = 1000 * 60 * 60 * 24 * 182;

export function PrimaryListForm({ item, chainId = CHAIN.POLYGON, onClose, now = Date.now }: { item: PublishableItem; chainId?: ChainId; onClose: () => void; now?: () => number }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const block = useWalletBlock("Putting an item on sale");
  const [credits, setCredits] = useState("");
  const write = useWrite();
  const value = Number(credits);
  const valid = /^\d+$/.test(credits.trim()) && Number.isSafeInteger(value) && value >= 1;

  if (block || !provider) return block;
  if (write.state.kind === "done")
    return (
      <div className="mk-section" data-testid="shop-primary-listed">
        <p role="status">
          “{item.name}” is on sale for {value.toLocaleString("en-US")} credits · {item.remainingSupply.toLocaleString("en-US")} available.
        </p>
        <div className="mk-actions">
          <Link className="mk-link" to={shopPath("itemDetail", { contractAddress: item.contractAddress, itemId: item.blockchainItemId })}>
            View in the Shop
          </Link>
          <Button variant="ghost" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>
      </div>
    );
  return (
    <form
      className="mk-panel mk-form"
      aria-label={`Put ${item.name} on sale`}
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        void write.run("Enabling sales for this collection…", "On sale.", async (progress) => {
          await ensureMinter({ provider, contractAddress: item.contractAddress, chainId });
          progress("Sign the listing in your wallet…");
          const trade = await createPrimaryUsdPeggedListing({
            provider,
            item: { contractAddress: item.contractAddress, itemId: item.blockchainItemId, chainId },
            usdCents: value * CENTS_PER_CREDIT,
            uses: item.remainingSupply,
            expiresAtMs: now() + SIX_MONTHS_MS,
          });
          progress("Publishing your item…");
          const created = await postListingWithRetry(trade);
          await qc.invalidateQueries({ queryKey: ["store-sale-state"] });
          await qc.invalidateQueries({ queryKey: ["collection-sale-state"] });
          return created.id;
        });
      }}
    >
      <h2 className="mk-panel__title">Publish “{item.name}”</h2>
      <p className="mk-hint">
        From your collection “{item.collectionName}” · {item.remainingSupply.toLocaleString("en-US")} available
      </p>
      <label className="mk-field">
        <span className="mk-field__label">Price (Credits)</span>
        <input inputMode="numeric" value={credits} onChange={(e) => setCredits(e.target.value)} placeholder="10" disabled={write.busy} aria-invalid={credits && !valid ? true : undefined} />
        {credits && !valid ? <p className="mk-field__error">Enter a whole number of Credits.</p> : null}
        <p className="mk-field__help">
          Priced in whole Credits (1 Credit = $0.10){valid ? `: ${formatUsd(usdCents(value * CENTS_PER_CREDIT))}` : ""}. Buyers pay with Credits, or with MANA from a web3 wallet; you are paid in MANA.
        </p>
      </label>
      <p className="mk-hint">The first listing from a collection asks you to let the Shop mint it (once, no gas); then you confirm the listing.</p>
      <div className="mk-actions">
        <Button type="submit" disabled={!valid || write.busy}>
          {write.busy ? "Publishing…" : "Put on sale"}
        </Button>
        <Button variant="ghost" disabled={write.busy} onClick={onClose}>
          Cancel
        </Button>
      </div>
      <WriteStatus state={write.state} />
    </form>
  );
}
