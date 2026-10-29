// from: marketplace@f3956826:webapp/src/components/Modals/{ClaimLandModal/ClaimLandModal,RemoveRentalModal/
// RemoveRentalModal,SubmitTransactionModal/SubmitTransactionModal}.tsx
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// Claim a LAND back from the Rentals contract, or remove a rental listing: one transaction each, paid
// in the network's gas by the owner. A transaction that is sent but not confirmed is shown as pending
// with its link, and never offered again.

import { useState } from "react";

import Button from "../../atoms/Button";
import Modal from "../../components/Modal";
import type { Eip1193Provider } from "../../data/auth/wallet";
import type { ShopClient } from "../api/shop";
import { txHref } from "./evm";
import { claimAsset, removeRental, type RentalListing, type RentalTxStage } from "./rentals";
import { txOutcome, type TxOutcome } from "./txText";
import "./land.css";

export type RentalTxKind = "claim" | "remove";

export type RentalTxModalProps = {
  kind: RentalTxKind;
  asset: { chainId: number; contractAddress: string; tokenId: string; openRentalId: string | null; category: "parcel" | "estate" };
  rental: RentalListing;
  account: string;
  provider: Eip1193Provider;
  client?: ShopClient;
  onClose(): void;
  onDone(rental: RentalListing): void;
  portal?: boolean;
  timing?: { poll?: { retries?: number; delayMs?: number }; wait?: { pollMs?: number; timeoutMs?: number } };
};

const STAGE_TEXT: Partial<Record<RentalTxStage, string>> = {
  "awaiting-confirmation": "Confirm the transaction in your wallet to continue.",
  confirming: "Waiting for the network to confirm the transaction…",
  indexing: "Confirmed. Updating the listing…",
};

export default function RentalTxModal({ kind, asset, rental, account, provider, client, onClose, onDone, portal, timing }: RentalTxModalProps) {
  const assetWord = asset.category === "estate" ? "Estate" : "Parcel";
  const [stage, setStage] = useState<RentalTxStage | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<TxOutcome | null>(null);
  const busy = stage !== null && outcome === null;
  const sent = txHash !== null;

  const title = kind === "claim" ? `Claim ${assetWord}` : "Remove listing";

  const go = async () => {
    setOutcome(null);
    setStage("awaiting-confirmation");
    const common = {
      provider,
      account,
      client,
      asset,
      onStage: (s: RentalTxStage, hash?: string) => {
        setStage(s);
        if (hash) setTxHash(hash);
      },
      poll: timing?.poll,
      wait: timing?.wait,
    };
    try {
      const out = kind === "claim" ? await claimAsset({ ...common, rental }) : await removeRental(common);
      setStage(null);
      onDone(out.rental);
    } catch (e) {
      setOutcome(txOutcome(e));
    }
  };

  return (
    <Modal onClose={busy ? undefined : onClose} closeOnBackdrop={!busy} closeOnEsc={!busy} showClose={!busy} width={440} portal={portal} ariaLabel={title}>
      <div className="shop-land-modal">
        <h2>{title}</h2>
        {kind === "claim" ? (
          <p>Are you sure you want to claim your {assetWord.toLowerCase()} back? It returns to your wallet and its rental listing ends.</p>
        ) : (
          <p>Are you sure you want to remove your listing? The signed listing stops being valid on-chain.</p>
        )}
        <p>This requires a transaction, and you will need to cover its network fee.</p>

        {busy && stage && STAGE_TEXT[stage] ? (
          <p role="status" aria-live="polite">
            {STAGE_TEXT[stage]}
          </p>
        ) : null}

        {outcome ? (
          <div className={`shop-land-note ${outcome.kind === "failed" ? "shop-land-note--error" : "shop-land-note--warn"}`} role={outcome.kind === "failed" ? "alert" : "status"}>
            <p>{outcome.text}</p>
          </div>
        ) : null}

        {txHash ? (
          <p>
            <a href={txHref(asset.chainId, txHash)} target="_blank" rel="noreferrer">
              View transaction
            </a>
          </p>
        ) : null}

        <div className="shop-land-actions">
          {!sent && (!outcome || outcome.kind === "declined" || (outcome.kind === "failed" && outcome.retry)) ? (
            <Button onClick={go} disabled={busy} tone={kind === "remove" ? "danger" : undefined}>
              {busy ? "Working…" : outcome ? "Try again" : title}
            </Button>
          ) : null}
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            {sent && outcome ? "Close" : "Cancel"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
