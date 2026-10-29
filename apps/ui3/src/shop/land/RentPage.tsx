// from: marketplace@f3956826:webapp/src/components/AssetPage/SaleRentActionBox/{SaleRentActionBox,
// PeriodsDropdown/PeriodsDropdown}.tsx, webapp/src/components/Modals/{ConfirmRentModal/ConfirmRentModal,
// RentConfirmedModal/RentConfirmedModal,RentConfirmedModal/CTA/CTA}.tsx, webapp/src/modules/rental/sagas.ts
// (ACCEPT_RENTAL_LISTING)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useParams } from "react-router";

import Button from "../../atoms/Button";
import type { ShopClient } from "../api/shop";
import { tokenAsset } from "../model/asset";
import { evmAddress } from "../model/address";
import { chainLabel } from "../model/chain";
import { manaWei, type ManaWei } from "../model/money";
import type { PurchaseOffer } from "../model/purchase";
import { present, restrictions } from "../payment/rule";
import { presentPayment, presentUnpriced } from "../payment/present";
import { optionStateText } from "../payment/copy";
import ManaGuidance, { type GuidanceRevalidation } from "../wallet/ManaGuidance";
import { BackLink, LandTokenDetails, LandTokenHighlights, LandTokenMap } from "../detail/components/LandBranch";
import { fetchNftResult, indexOutage, landData } from "./api";
import { landCategoryOf, landNetwork } from "./contracts";
import { erc20Abi, readContract, rpcReader, txHref, type ChainReader } from "./evm";
import { Mana, shortDateText } from "./format";
import { BUILDER_LAND_HREF } from "./ManageAsset";
import { acceptRentalListing, isOwnedBy, isRentalExecuted, isRentalOpen, pricePerRent, rentalEndDate, type RentalListing, type RentalTxStage } from "./rentals";
import { fetchTiles, type TilesResult } from "./tiles";
import { txOutcome, type TxOutcome } from "./txText";
import { useLandWallet } from "./useLandWallet";
import { useResource } from "./useResource";
import "./land.css";

const DAY_MS = 86_400_000;

export function rentalOffer(rental: RentalListing, periodIndex: number | null, session: Parameters<typeof present>[1], now: number = Date.now()): PurchaseOffer {
  const asset = tokenAsset(rental.chainId, evmAddress(rental.contractAddress), rental.tokenId);
  const eligibility = present(restrictions("rental", "secondary", rental.chainId, false), session, rental.chainId);
  const period = periodIndex === null ? null : rental.periods[periodIndex];
  const venue = evmAddress(landNetwork(rental.chainId).rentals);
  return {
    asset,
    kind: "rental",
    sale: "secondary",
    bundle: false,
    eligibility,
    quotes: period ? [{ method: "mana", amount: manaWei(pricePerRent(period)) }] : [],
    unavailable: [],
    settlement: [{ method: "mana", key: { method: "mana", chainId: rental.chainId, contract: venue, call: { kind: "one", asset } } }],
    available: null,
    quotedAt: now,
  };
}

export type RentPageProps = {
  client?: ShopClient;
  loadTiles?: (signal: AbortSignal) => Promise<TilesResult>;
  reader?: ChainReader;
  contractAddress?: string;
  tokenId?: string;
  now?: () => number;
  timing?: { poll?: { retries?: number; delayMs?: number }; wait?: { pollMs?: number; timeoutMs?: number } };
};

type Phase =
  | { kind: "select" }
  | { kind: "confirm" }
  | { kind: "renting"; stage: RentalTxStage; txHash: string | null }
  | { kind: "done"; rental: RentalListing; txHash: string }
  | { kind: "outcome"; outcome: TxOutcome; txHash: string | null };

const STAGE_TEXT: Record<RentalTxStage, string> = {
  checking: "Checking your MANA allowance…",
  "awaiting-approval": "Approve MANA for the Rentals contract in your wallet.",
  approving: "Waiting for the MANA approval to confirm…",
  "awaiting-confirmation": "Confirm the rental in your wallet.",
  confirming: "Waiting for the network to confirm the rental…",
  indexing: "Confirmed. Updating the listing…",
};

export default function RentPage(props: RentPageProps) {
  const params = useParams();
  const location = useLocation();
  const contractAddress = (props.contractAddress ?? params.contractAddress ?? "").toLowerCase();
  const tokenId = props.tokenId ?? params.tokenId ?? "";
  const wallet = useLandWallet();
  const land = useMemo(() => landCategoryOf(contractAddress), [contractAddress]);
  const asset = useResource((signal) => fetchNftResult(contractAddress, tokenId, { client: props.client, signal }), [contractAddress, tokenId, props.client, wallet.revision]);
  const tiles = useResource<TilesResult>(land ? (signal) => (props.loadTiles ? props.loadTiles(signal) : fetchTiles({ signal })) : null, [!!land]);
  const [pickedPeriod, setPeriodIndex] = useState<number | null>(() => {
    const raw = new URLSearchParams(location.search).get("period");
    const n = raw === null || raw === "" ? NaN : Number(raw);
    return Number.isInteger(n) && n >= 0 ? n : null;
  });
  const [phase, setPhase] = useState<Phase>({ kind: "select" });
  const now = (props.now ?? Date.now)();
  const tokenHref = `/shop/token/${contractAddress}/${tokenId}`;
  const here = `${location.pathname}${location.search}`;

  const result = asset.data;
  const rental = result?.rental ?? null;
  const periodIndex = rental && pickedPeriod !== null && pickedPeriod < rental.periods.length ? pickedPeriod : null;
  const chainId = result?.nft.chainId ?? land?.network.chainId ?? 1;

  const seenPeriods = useRef<string | null>(null);
  const seenRevision = useRef(wallet.revision);
  const [revalidation, setRevalidation] = useState<GuidanceRevalidation["status"] | null>(null);
  useEffect(() => {
    if (wallet.revision !== seenRevision.current) {
      seenRevision.current = wallet.revision;
      setRevalidation("checking");
      setPhase((p) => (p.kind === "confirm" ? { kind: "select" } : p));
    }
  }, [wallet.revision]);
  useEffect(() => {
    const key = rental ? JSON.stringify([rental.status, rental.periods, rental.nonces]) : null;
    if (asset.status === "ready") {
      if (revalidation === "checking") setRevalidation(seenPeriods.current === null || seenPeriods.current === key ? "current" : "changed");
      seenPeriods.current = key;
    } else if (asset.status === "error" && revalidation === "checking") {
      setRevalidation("failed");
    }
  }, [asset.status, rental, revalidation]);

  const reader = useMemo(() => props.reader ?? (land ? rpcReader(chainId) : null), [props.reader, land, chainId]);
  const balance = useResource<bigint | null>(
    wallet.session.kind === "web3" && wallet.account && reader && land
      ? (signal) => readContract<bigint>(reader, landNetwork(chainId).mana, erc20Abi, "balanceOf", [wallet.account], signal)
      : null,
    [wallet.account, wallet.session.kind, chainId, reader],
  );

  if (asset.status === "loading" && !result) {
    return (
      <div className="shop-land-page" aria-busy="true">
        <p role="status">Loading…</p>
      </div>
    );
  }
  if (asset.status === "error" && !result) {
    const outage = indexOutage(asset.error);
    return (
      <div className="shop-land-page">
        <div className={`shop-land-note ${outage ? "shop-land-note--warn" : "shop-land-note--error"}`} role="alert">
          <p>{outage ? `This LAND cannot be shown right now: ${outage.message}` : "This LAND could not be loaded."}</p>
          <Button size="sm" variant="secondary" onClick={asset.reload}>
            Try again
          </Button>
        </div>
      </div>
    );
  }
  if (!result || !land) {
    return (
      <div className="shop-land-page">
        <p className="shop-land-page__sub">{result ? "Only LAND and estates can be rented." : "Not found…"}</p>
      </div>
    );
  }

  const d = landData(result);
  const back = <BackLink to={tokenHref} />;

  if (phase.kind === "done") {
    const end = rentalEndDate(phase.rental) ?? new Date(now + (rental?.periods[periodIndex ?? 0]?.maxDays ?? 0) * DAY_MS);
    return (
      <div className="shop-landtoken shop-landtoken--rent">
        {back}
        <section className="shop-land-box" aria-labelledby="shop-rent-done">
          <h1 id="shop-rent-done">LAND rented</h1>
          <p>Awesome! You've just rented {result.nft.name} until {shortDateText(end.getTime())}.</p>
          <p>
            <a href={txHref(chainId, phase.txHash)} target="_blank" rel="noreferrer">
              View transaction
            </a>
          </p>
          <ul className="shop-land-cta-list">
            <li>
              <a href="/create/scenes">
                Get creative with the Scene Builder<span>Easily create simpler scenes.</span>
              </a>
            </li>
            <li>
              <a href="https://docs.decentraland.org/creator/scene-editor/extend-with-code/overview" target="_blank" rel="noreferrer">
                Build more complex scenes with our SDK<span>Ideal if you have some technical background.</span>
              </a>
            </li>
            <li>
              <a href={BUILDER_LAND_HREF}>
                Manage LAND<span>Add an existing scene, or review permissions.</span>
              </a>
            </li>
          </ul>
        </section>
      </div>
    );
  }

  const open = isRentalOpen(rental);
  const partOfEstate = d.kind === "parcel" && !!d.estate;
  const isOwner = !!wallet.account && isOwnedBy({ chainId, contractAddress, tokenId, owner: result.nft.owner }, wallet.account, rental);
  const offer = rental ? rentalOffer(rental, periodIndex, wallet.session, now) : null;
  const view = !offer
    ? null
    : periodIndex === null
      ? presentUnpriced(offer.eligibility)
      : presentPayment(offer, wallet.session, { manaBalance: balance.data != null ? (balance.data as ManaWei) : null });
  const manaOption = view?.options.find((o) => o.method === "mana") ?? null;
  const total = rental && periodIndex !== null ? pricePerRent(rental.periods[periodIndex]!) : null;
  const wrongNetwork = !!wallet.provider && !wallet.onChain(chainId);
  const canRent =
    open && !partOfEstate && !isOwner && periodIndex !== null && wallet.session.kind === "web3" && !!wallet.signer && !wrongNetwork && manaOption?.state.kind === "ready" && revalidation !== "checking";

  return (
    <div className="shop-landtoken shop-landtoken--rent" data-testid="land-rent-page">
      {back}
      <LandTokenMap result={result} tiles={tiles.data?.tiles ?? null} />
      <div className="shop-landtoken__container">
        <div className="shop-landtoken__info">
          <div className="shop-landtoken__left">
            <h1 className="shop-landtoken__title" id="shop-rent-title">
              Rent {result.nft.name}
            </h1>
            <LandTokenHighlights result={result} tiles={tiles.data?.tiles ?? null} />
            <section className="shop-land-box" aria-labelledby="shop-rent-title">
              {!open ? (
                <p>
                  {isRentalExecuted(rental) && rental && rentalEndDate(rental)
                    ? `Rented until ${shortDateText(rentalEndDate(rental)!.getTime())}. It is not listed for rent right now.`
                    : "This LAND is not listed for rent right now."}{" "}
                  <Link to={tokenHref}>See its page</Link>
                </p>
              ) : partOfEstate ? (
                <p>It's not possible to rent a parcel that belongs to an Estate. Rent the Estate instead from its page.</p>
              ) : isOwner ? (
                <p>
                  This is your LAND. <Link to={`${tokenHref}/manage`}>Manage its rental listing</Link>
                </p>
              ) : rental ? (
                <>
                  <fieldset className="shop-land-periods" disabled={phase.kind === "renting"}>
                    <legend>Select a rental period</legend>
                    {rental.periods.map((p, i) => (
                      <label key={`${p.minDays}-${p.maxDays}-${i}`}>
                        <span>
                          {p.maxDays} days
                          <small>
                            From {shortDateText(now)} to {shortDateText(now + p.maxDays * DAY_MS)} · <Mana wei={p.pricePerDay} suffix="/day" />
                          </small>
                        </span>
                        <span>
                          <Mana wei={pricePerRent(p)} />{" "}
                          <input
                            type="radio"
                            name="shop-rent-period"
                            aria-label={`${p.maxDays} days`}
                            checked={periodIndex === i}
                            onChange={() => {
                              setPeriodIndex(i);
                              setPhase({ kind: "select" });
                            }}
                          />
                        </span>
                      </label>
                    ))}
                  </fieldset>
                  <p className="shop-land-page__sub">Paid in MANA on {chainLabel(chainId)}. 2.5% of the rent goes to the DAO.</p>
                </>
              ) : null}
            </section>

            {open && !partOfEstate && !isOwner && view ? (
              wallet.session.kind !== "web3" ? (
                <ManaGuidance
                  guidance={view.guidance!}
                  required={total !== null ? (total as ManaWei) : null}
                  reasons={view.reasons}
                  item={{ name: result.nft.name, href: tokenHref }}
                  returnTo={here}
                  onSignIn={wallet.host?.onSignIn}
                  onConnectWallet={wallet.host?.onConnectWallet}
                  onSwitchAccount={wallet.host?.onSwitchAccount}
                  revalidation={revalidation ? { status: revalidation, onRetry: asset.reload } : null}
                />
              ) : (
                <section className="shop-land-box" aria-labelledby="shop-rent-pay">
                  <h2 id="shop-rent-pay">Pay with MANA</h2>
                  {wallet.accountMismatch ? (
                    <div className="shop-land-note shop-land-note--error" role="alert">
                      <p>
                        Your wallet switched to {wallet.walletAccount}, but you are signed in as {wallet.account}. Sign in again with the wallet you want to rent with; the
                        LAND will be rented to that account.
                      </p>
                      {wallet.host?.onSwitchAccount ? (
                        <Button size="sm" onClick={() => wallet.host?.onSwitchAccount?.()}>
                          Sign in with {wallet.walletAccount}
                        </Button>
                      ) : null}
                    </div>
                  ) : wrongNetwork ? (
                    <div className="shop-land-note shop-land-note--warn" role="status">
                      <p>
                        Your wallet is on {wallet.chainId ? chainLabel(wallet.chainId) : "another network"}. Renting runs on {chainLabel(chainId)}.
                      </p>
                      <Button size="sm" variant="secondary" disabled={wallet.pendingChainId !== null} onClick={() => void wallet.switchTo(chainId)}>
                        {wallet.pendingChainId !== null ? "Confirm in your wallet…" : `Switch to ${chainLabel(chainId)}`}
                      </Button>
                    </div>
                  ) : null}
                  {revalidation === "checking" ? (
                    <p role="status">Checking the price, availability and ownership again for this wallet…</p>
                  ) : revalidation === "changed" ? (
                    <p role="alert">The listing changed after the switch. Review the periods before renting.</p>
                  ) : null}
                  {total !== null ? (
                    <div className="shop-land-box__cols">
                      <div className="shop-land-box__col">
                        <span>Total</span>
                        <Mana wei={total} />
                      </div>
                      {balance.data != null ? (
                        <div className="shop-land-box__col">
                          <span>Your balance on {chainLabel(chainId)}</span>
                          <Mana wei={balance.data} />
                        </div>
                      ) : null}
                    </div>
                  ) : (
                    <p className="shop-land-page__sub">Select a period to see the total.</p>
                  )}
                  {manaOption && manaOption.state.kind !== "ready" && total !== null ? (
                    <p role="status">{manaOption.state.kind === "insufficientMana" ? "You don't have enough MANA to rent this LAND for that period." : optionStateText("mana", manaOption.state)}</p>
                  ) : null}
                  {phase.kind === "select" ? (
                    <div className="shop-land-actions">
                      <Button onClick={() => setPhase({ kind: "confirm" })} disabled={!canRent}>
                        Rent
                      </Button>
                    </div>
                  ) : null}
                  {phase.kind === "confirm" && rental && periodIndex !== null && wallet.provider && wallet.signer ? (
                    <ConfirmRent
                      name={result.nft.name}
                      rental={rental}
                      periodIndex={periodIndex}
                      account={wallet.signer}
                      now={now}
                      onCancel={() => setPhase({ kind: "select" })}
                      onConfirm={async (operator) => {
                        const provider = wallet.provider!;
                        let hash: string | null = null;
                        setPhase({ kind: "renting", stage: "checking", txHash: null });
                        try {
                          const out = await acceptRentalListing({
                            provider,
                            account: wallet.signer!,
                            asset: { chainId, contractAddress, tokenId, openRentalId: result.nft.openRentalId ?? rental.id },
                            rental,
                            periodIndex,
                            operator,
                            agreedWei: pricePerRent(rental.periods[periodIndex]!),
                            client: props.client,
                            reader: props.reader,
                            poll: props.timing?.poll,
                            wait: props.timing?.wait,
                            onSubmitted: (h) => (hash = h),
                            onStage: (stage, h) => setPhase({ kind: "renting", stage, txHash: h ?? hash }),
                          });
                          setPhase({ kind: "done", rental: out.rental, txHash: out.txHash });
                        } catch (e) {
                          const outcome = txOutcome(e);
                          setPhase({ kind: "outcome", outcome, txHash: hash });
                        }
                      }}
                    />
                  ) : null}
                  {phase.kind === "renting" ? (
                    <p role="status" aria-live="polite">
                      {STAGE_TEXT[phase.stage]}
                    </p>
                  ) : null}
                  {phase.kind === "outcome" ? (
                    <div className={`shop-land-note ${phase.outcome.kind === "failed" ? "shop-land-note--error" : "shop-land-note--warn"}`} role={phase.outcome.kind === "failed" ? "alert" : "status"}>
                      <p>{phase.outcome.text}</p>
                      {phase.txHash ? (
                        <a href={txHref(chainId, phase.txHash)} target="_blank" rel="noreferrer">
                          View transaction
                        </a>
                      ) : null}
                      {phase.outcome.kind === "declined" || (phase.outcome.kind === "failed" && phase.outcome.retry && !phase.txHash) ? (
                        <Button size="sm" variant="secondary" onClick={() => setPhase({ kind: "select" })}>
                          Try again
                        </Button>
                      ) : null}
                      {phase.outcome.kind === "indexing" || phase.outcome.kind === "pending" ? (
                        <Button size="sm" variant="secondary" onClick={asset.reload}>
                          Refresh
                        </Button>
                      ) : null}
                    </div>
                  ) : null}
                </section>
              )
            ) : null}
          </div>
          <div className="shop-landtoken__right">
            <LandTokenDetails result={result} />
          </div>
        </div>
      </div>
    </div>
  );
}

function ConfirmRent({
  name,
  rental,
  periodIndex,
  account,
  now,
  onCancel,
  onConfirm,
}: {
  name: string;
  rental: RentalListing;
  periodIndex: number;
  account: string;
  now: number;
  onCancel(): void;
  onConfirm(operator: string): void;
}) {
  const period = rental.periods[periodIndex]!;
  const total = pricePerRent(period);
  const [price, setPrice] = useState("");
  const [self, setSelf] = useState(true);
  const [operator, setOperator] = useState(account);
  const totalText = (() => {
    const whole = total / 10n ** 18n;
    const frac = (total % 10n ** 18n).toString().padStart(18, "0").replace(/0+$/, "");
    return frac ? `${whole}.${frac}` : whole.toString();
  })();
  const priceMatches = price.trim() !== "" && Number(price) === Number(totalText);
  const operatorValid = /^0x[0-9a-fA-F]{40}$/.test(operator) && !/^0x0{40}$/.test(operator);
  return (
    <form
      className="shop-land-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (priceMatches && operatorValid) onConfirm(operator.toLowerCase());
      }}
    >
      <p>
        You are about to rent <b>{name}</b> for <b>{period.maxDays}</b> days (from {shortDateText(now)} to {shortDateText(now + period.maxDays * DAY_MS)}) for a total price of{" "}
        <Mana wei={total} />.
      </p>
      <p>Please re-enter the price and the address who will have the operator permission to operate the LAND:</p>
      <label>
        Price (MANA)
        <input className="shop-land-input" inputMode="decimal" placeholder={totalText} value={price} onChange={(e) => setPrice(e.target.value.replace(/[^0-9.]/g, ""))} />
      </label>
      <label>
        Operator address
        <input
          className="shop-land-input"
          value={operator}
          disabled={self}
          aria-invalid={!operatorValid}
          onChange={(e) => setOperator(e.target.value.trim())}
        />
        {!operatorValid ? <small role="alert">Please insert a valid address</small> : null}
      </label>
      <label className="shop-land-check">
        <input
          type="checkbox"
          checked={self}
          onChange={() => {
            setSelf((s) => !s);
            setOperator(self ? "" : account);
          }}
        />{" "}
        I will be the operator of this LAND
      </label>
      <div className="shop-land-actions">
        <Button type="submit" disabled={!priceMatches || !operatorValid}>
          Confirm
        </Button>
        <Button variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
