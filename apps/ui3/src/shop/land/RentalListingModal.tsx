// from: marketplace@f3956826:webapp/src/components/Modals/RentalListingModal/{RentalListingModal,
// AuthorizationStep/AuthorizationStep,InformationStep/InformationStep,CreateOrEditListingStep/CreateOrEditListingStep,
// ConfirmationStep/ConfirmationStep,EditConfirmationStep/EditConfirmationStep}.tsx
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// List a LAND or an estate for rent, or edit its listing. The Rentals contract must first be approved
// as an operator of the registry (as for selling); editing an open listing is two steps, removing the
// old signature on-chain and signing the new one. Rent is priced in MANA per day; nothing here is
// priced in credits.

import { useEffect, useMemo, useState } from "react";

import Button from "../../atoms/Button";
import Modal from "../../components/Modal";
import type { Eip1193Provider } from "../../data/auth/wallet";
import type { ShopClient } from "../api/shop";
import { landCategoryOf, landNetwork } from "./contracts";
import { addressHref, ensureOperatorApproval, erc721Abi, readContract, walletReader, type ChainReader } from "./evm";
import { Mana } from "./format";
import {
  daysByPeriod,
  isRentalOpen,
  listingPeriods,
  maxPriceOfPeriods,
  PERIOD_LABEL,
  PERIOD_OPTIONS,
  periodsByDays,
  removeRental,
  RENTAL_MIN_PRICE_MANA,
  upsertRental,
  type PeriodOption,
  type RentalListing,
} from "./rentals";
import { txOutcome } from "./txText";
import "./land.css";

export type RentalListingAsset = { chainId: number; contractAddress: string; tokenId: string; openRentalId: string | null; name: string };

export type RentalListingModalProps = {
  asset: RentalListingAsset;
  /** The open listing to edit, or the claimable one to list again; null for a first listing. */
  rental: RentalListing | null;
  /** The LAND can be claimed back and has no open listing: list it again at no cost. */
  listForRentAgain?: boolean;
  account: string;
  provider: Eip1193Provider;
  client?: ShopClient;
  reader?: ChainReader;
  onClose(): void;
  onDone(rental: RentalListing): void;
  /** Remove the open listing instead (the separate remove flow). */
  onRemove?(): void;
  portal?: boolean;
  /** Poll and receipt tuning, for tests. */
  timing?: { poll?: { retries?: number; delayMs?: number }; wait?: { pollMs?: number; timeoutMs?: number } };
};

type Step = "checking" | "authorize" | "again" | "form" | "confirm" | "editConfirm";

const DAY_MS = 86_400_000;

function dateInput(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function dateInputMs(value: string): number {
  return new Date(`${value}T00:00:00`).getTime();
}

function weiToMana(wei: string): string {
  const w = BigInt(wei);
  const whole = w / 10n ** 18n;
  const frac = (w % 10n ** 18n).toString().padStart(18, "0").replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : whole.toString();
}

export default function RentalListingModal({
  asset,
  rental,
  listForRentAgain = false,
  account,
  provider,
  client,
  reader,
  onClose,
  onDone,
  onRemove,
  portal,
  timing,
}: RentalListingModalProps) {
  const editing = isRentalOpen(rental);
  const category = landCategoryOf(asset.contractAddress)?.category ?? "parcel";
  const assetWord = category === "estate" ? "Estate" : "parcel";
  const [step, setStep] = useState<Step>("checking");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stepOneDone, setStepOneDone] = useState(false);

  const oldPrice = rental ? weiToMana(maxPriceOfPeriods(rental)) : "";
  const oldPeriods = useMemo(() => (rental ? rental.periods.map((p) => periodsByDays[p.maxDays]).filter((p): p is PeriodOption => !!p) : []), [rental]);
  const oldExpiry = rental && editing ? dateInput(rental.expiration) : null;
  const [price, setPrice] = useState(oldPrice);
  const [periods, setPeriods] = useState<PeriodOption[]>(oldPeriods);
  const [expiry, setExpiry] = useState(oldExpiry ?? dateInput(Date.now() + 30 * DAY_MS));

  useEffect(() => {
    let live = true;
    const r = reader ?? walletReader(provider, asset.chainId);
    readContract<boolean>(r, asset.contractAddress, erc721Abi, "isApprovedForAll", [account, landNetwork(asset.chainId).rentals]).then(
      (approved) => live && setStep(approved ? (listForRentAgain && !editing ? "again" : "form") : "authorize"),
      () => live && setStep("authorize"),
    );
    return () => {
      live = false;
    };
  }, [account, asset.chainId, asset.contractAddress, provider, reader, listForRentAgain, editing]);

  let periodsForListing: ReturnType<typeof listingPeriods> | null = null;
  let priceError: string | null = null;
  try {
    periodsForListing = price.trim() ? listingPeriods(price, periods.length ? periods : [PERIOD_OPTIONS[0]!]) : null;
  } catch (e) {
    priceError = (e as Error).message;
  }
  const expiryMs = dateInputMs(expiry);
  const expiryInvalid = !Number.isFinite(expiryMs) || expiryMs < Date.now();
  const invalid = !periodsForListing || !!priceError || periods.length === 0 || expiryInvalid;
  const changed =
    !editing ||
    oldExpiry !== expiry ||
    price.trim() !== oldPrice ||
    periods.length !== oldPeriods.length ||
    !oldPeriods.every((p) => periods.includes(p));

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      const out = txOutcome(e);
      setError(out.text);
    } finally {
      setBusy(false);
    }
  };

  const authorize = () =>
    run(async () => {
      await ensureOperatorApproval(provider, {
        chainId: asset.chainId,
        registry: asset.contractAddress,
        owner: account,
        operator: landNetwork(asset.chainId).rentals,
        reader,
        wait: timing?.wait,
      });
      setStep(listForRentAgain && !editing ? "again" : "form");
    });

  const sign = () =>
    run(async () => {
      const created = await upsertRental({
        provider,
        account,
        asset: { chainId: asset.chainId, contractAddress: asset.contractAddress, tokenId: asset.tokenId },
        pricePerDay: price,
        periods,
        expiresAt: expiryMs,
        client,
        reader,
      });
      onDone(created);
    });

  const removeOld = () =>
    run(async () => {
      await removeRental({
        provider,
        account,
        asset: { chainId: asset.chainId, contractAddress: asset.contractAddress, tokenId: asset.tokenId, openRentalId: asset.openRentalId },
        client,
        poll: timing?.poll,
        wait: timing?.wait,
      });
      setStepOneDone(true);
    });

  const title =
    step === "authorize"
      ? "List for Rent"
      : step === "again"
        ? "List for Rent Again"
        : step === "confirm"
          ? "Confirm Listing"
          : step === "editConfirm"
            ? "Edit Listing"
            : listForRentAgain
              ? "List for Rent Again"
              : editing
                ? "Edit listing"
                : "List for Rent";
  const closable = !busy && !(step === "editConfirm" && stepOneDone);

  return (
    <Modal onClose={closable ? onClose : undefined} closeOnBackdrop={closable} closeOnEsc={closable} showClose={closable} width={480} portal={portal} ariaLabel={title}>
      <div className="shop-land-modal">
        <h2>{title}</h2>

        {step === "checking" ? <p role="status">Checking your wallet's approvals…</p> : null}

        {step === "authorize" ? (
          <>
            <p>
              Before listing your {assetWord} for rent you need to approve the{" "}
              <a href={addressHref(asset.chainId, landNetwork(asset.chainId).rentals)} target="_blank" rel="noreferrer">
                Rentals
              </a>{" "}
              contract so it can operate your {assetWord}, exactly like when selling it.
            </p>
            <p>When the renting period is over you will have two options:</p>
            <ul>
              <li>
                <b>Claim your LAND back so you can use it</b>: you will need to send a new transaction and cover its cost.
              </li>
              <li>
                <b>List your LAND for rent again</b>: this is free of charge.
              </li>
            </ul>
            <div className="shop-land-actions">
              <Button onClick={authorize} disabled={busy}>
                {busy ? "Confirm in your wallet…" : "Proceed"}
              </Button>
              <Button variant="secondary" onClick={onClose} disabled={busy}>
                Cancel
              </Button>
            </div>
          </>
        ) : null}

        {step === "again" ? (
          <>
            <p>List this {assetWord} for rent again at no cost.</p>
            <p>The current tenant will hold operator permissions until it is rented again.</p>
            <p>To retake operator permissions, claim it back (network fees involved).</p>
            <div className="shop-land-actions">
              <Button onClick={() => setStep("form")}>List for Rent Again</Button>
              <Button variant="secondary" onClick={onClose}>
                Cancel
              </Button>
            </div>
          </>
        ) : null}

        {step === "form" ? (
          <form
            className="shop-land-form"
            onSubmit={(e) => {
              e.preventDefault();
              if (!invalid && changed) setStep(editing ? "editConfirm" : "confirm");
            }}
          >
            {editing ? <p>Editing the listing requires a new transaction, and you will need to pay its network fee.</p> : null}
            <label>
              Price per day (MANA)
              <input
                className="shop-land-input"
                inputMode="decimal"
                placeholder="1000"
                value={price}
                maxLength={20}
                aria-invalid={!!priceError && price !== ""}
                onChange={(e) => setPrice(e.target.value.replace(/[^0-9.]/g, ""))}
              />
              {price !== "" && priceError ? <small role="alert">{priceError}</small> : <small>2.5% of the price goes to the DAO. The minimum is {RENTAL_MIN_PRICE_MANA} MANA per day.</small>}
            </label>
            <fieldset className="shop-land-periods">
              <legend>Rent periods (the days you are willing to rent your LAND for)</legend>
              {PERIOD_OPTIONS.map((p) => (
                <label key={p}>
                  <span>{PERIOD_LABEL[p]}</span>
                  <input
                    type="checkbox"
                    checked={periods.includes(p)}
                    onChange={() =>
                      setPeriods((cur) => (cur.includes(p) ? cur.filter((x) => x !== p) : [...cur, p].sort((a, b) => daysByPeriod[a] - daysByPeriod[b])))
                    }
                  />
                </label>
              ))}
            </fieldset>
            <label>
              Listing expiration date
              <input className="shop-land-input" type="date" value={expiry} onChange={(e) => setExpiry(e.target.value || dateInput(Date.now() + 30 * DAY_MS))} aria-invalid={expiryInvalid} />
              {expiryInvalid ? <small role="alert">Invalid date</small> : <small>If no one rents your LAND, the listing is removed on this date.</small>}
            </label>
            <div className="shop-land-actions">
              <Button type="submit" disabled={invalid || !changed}>
                {listForRentAgain ? "List for Rent Again" : editing ? "Update listing" : "List for rent"}
              </Button>
              {editing && onRemove ? (
                <Button variant="secondary" onClick={onRemove}>
                  Remove listing
                </Button>
              ) : (
                <Button variant="secondary" onClick={onClose}>
                  Cancel
                </Button>
              )}
            </div>
          </form>
        ) : null}

        {step === "confirm" && periodsForListing ? (
          <>
            <p>
              Review and confirm the price and rent periods you selected.{" "}
              <b>Listing is free of charge, but editing or cancelling a listing requires sending a transaction, and you will need to cover its cost.</b>
            </p>
            <dl className="shop-land-dl">
              <div>
                <dt>Price per day</dt>
                <dd>
                  <Mana wei={periodsForListing[0]!.pricePerDay} suffix="/day" />
                </dd>
              </div>
              <div>
                <dt>Periods</dt>
                <dd>{periodsForListing.map((p) => p.maxDays).join(" / ")} days</dd>
              </div>
              <div>
                <dt>Listing expiration date</dt>
                <dd>{new Date(expiryMs).toLocaleDateString("en-US")}</dd>
              </div>
            </dl>
            <div className="shop-land-actions">
              <Button onClick={sign} disabled={busy}>
                {busy ? "Sign in your wallet…" : "Confirm"}
              </Button>
              <Button variant="secondary" onClick={() => setStep("form")} disabled={busy}>
                Back
              </Button>
            </div>
          </>
        ) : null}

        {step === "editConfirm" ? (
          <>
            <p>To update the listing, complete these actions:</p>
            <ol className="shop-land-steps">
              <li aria-current={!stepOneDone ? "step" : undefined} data-done={stepOneDone}>
                1. Remove the previous listing (for security reasons).{" "}
                {stepOneDone ? (
                  <span>Done</span>
                ) : (
                  <Button size="sm" onClick={removeOld} disabled={busy}>
                    {busy ? "Confirming…" : "Confirm transaction"}
                  </Button>
                )}
              </li>
              <li aria-current={stepOneDone ? "step" : undefined}>
                2. Sign the new listing.{" "}
                <Button size="sm" onClick={sign} disabled={!stepOneDone || busy}>
                  {busy && stepOneDone ? "Sign in your wallet…" : "Sign message"}
                </Button>
              </li>
            </ol>
            {!stepOneDone ? (
              <div className="shop-land-actions">
                <Button variant="secondary" onClick={() => setStep("form")} disabled={busy}>
                  Back
                </Button>
              </div>
            ) : null}
          </>
        ) : null}

        {error ? (
          <div className="shop-land-note shop-land-note--error" role="alert">
            <p>{error}</p>
          </div>
        ) : null}
      </div>
    </Modal>
  );
}
