// from: marketplace@f3956826:webapp/src/components/ManageAssetPage/{ManageAssetPage,Sell/Sell,Rent/Rent,
// IconButton/IconButton}.tsx, webapp/src/components/LandLockedPopup/LandLockedPopup.tsx,
// catalyrst-v2 ui3/src/marketplace/pages/MkManageAssetPage.tsx (moved here)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useMemo, useState } from "react";
import { Link, useParams } from "react-router";

import Button from "../../atoms/Button";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { ShopClient } from "../api/shop";
import { chainLabel, gasTokenSymbol } from "../model/chain";
import { fetchNftResult, indexOutage, landData } from "./api";
import { landCategoryOf } from "./contracts";
import { dateText, epochMs, Mana, relativeText } from "./format";
import {
  EstateParcels,
  EstateUpgradeWarning,
  LandDetails,
  LandHighlights,
  LandMapPreview,
  RentalHistory,
} from "./LandSections";
import RentalListingModal from "./RentalListingModal";
import SignInGate from "./SignInGate";
import RentalTxModal, { type RentalTxKind } from "./RentalTxModal";
import {
  canBeClaimed,
  hasRentalEnded,
  isLandLocked,
  isOwnedBy,
  isRentalCancelled,
  isRentalExecuted,
  isRentalOpen,
  maxPriceOfPeriods,
  rentalEndDate,
  sortPeriods,
  type RentalListing,
} from "./rentals";
import { fetchTiles, type TilesResult } from "./tiles";
import { useLandWallet } from "./useLandWallet";
import { useResource } from "./useResource";
import "./land.css";
import "./landgate.css";

function ManageSignIn({ onSignIn }: { onSignIn?: () => void }) {
  return (
    <div className="shop-land-gate">
      <SignInGate onSignIn={onSignIn} />
    </div>
  );
}

export const BUILDER_LAND_HREF = "/create/land";
export const BUILDER_NAMES_HREF = "/create/names";

export type ManageAssetProps = {
  client?: ShopClient;
  loadTiles?: (signal: AbortSignal) => Promise<TilesResult>;
  contractAddress?: string;
  tokenId?: string;
  now?: () => number;
  portal?: boolean;
  timing?: { poll?: { retries?: number; delayMs?: number }; wait?: { pollMs?: number; timeoutMs?: number } };
};

type Dialog = { kind: "list"; again: boolean } | { kind: RentalTxKind } | null;

export default function ManageAsset(props: ManageAssetProps) {
  const params = useParams();
  const contractAddress = (props.contractAddress ?? params.contractAddress ?? "").toLowerCase();
  const tokenId = props.tokenId ?? params.tokenId ?? "";
  const wallet = useLandWallet();
  const asset = useResource((signal) => fetchNftResult(contractAddress, tokenId, { client: props.client, signal }), [contractAddress, tokenId, props.client, wallet.revision]);
  const land = useMemo(() => landCategoryOf(contractAddress), [contractAddress]);
  const tiles = useResource<TilesResult>(land ? (signal) => (props.loadTiles ? props.loadTiles(signal) : fetchTiles({ signal })) : null, [!!land]);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const now = (props.now ?? Date.now)();

  const tokenHref = `/shop/token/${contractAddress}/${tokenId}`;

  if (land && !wallet.account) return <ManageSignIn onSignIn={wallet.host?.onSignIn} />;
  if (asset.status === "loading" && !asset.data) {
    return (
      <div className="shop-land-page" aria-busy="true">
        <p role="status">Loading…</p>
      </div>
    );
  }
  if (asset.status === "error") {
    const outage = indexOutage(asset.error);
    return (
      <div className="shop-land-page">
        <div className={`shop-land-note ${outage ? "shop-land-note--warn" : "shop-land-note--error"}`} role="alert">
          <p>{outage ? `This asset cannot be shown right now: ${outage.message}` : "This asset could not be loaded."}</p>
          <Button size="sm" variant="secondary" onClick={asset.reload}>
            Try again
          </Button>
        </div>
      </div>
    );
  }
  const result = asset.data;
  if (!result) {
    return (
      <div className="shop-land-page">
        <p className="shop-land-page__sub">Not found…</p>
      </div>
    );
  }

  if (!land) return <NonLandManage result={result} tokenHref={tokenHref} />;

  const rental = result.rental;
  const owner = { chainId: result.nft.chainId, contractAddress, tokenId, owner: result.nft.owner };
  const account = wallet.account;
  if (!account) return <ManageSignIn onSignIn={wallet.host?.onSignIn} />;
  if (!isOwnedBy(owner, account, rental)) {
    return (
      <div className="shop-land-page">
        <div className="shop-land-note shop-land-note--error" role="alert">
          <p>You are not the owner of {result.nft.name}, so you cannot manage it.</p>
          <Link to={tokenHref}>View {result.nft.name}</Link>
        </div>
      </div>
    );
  }

  const locked = rental ? isLandLocked(account, rental, owner, now) : false;
  const claimable = rental ? canBeClaimed(account, rental, owner, now) : false;
  const listAgain = !!rental && claimable && !isRentalOpen(rental);
  const d = landData(result);
  const assetWord = d.kind === "estate" ? "Estate" : "Parcel";
  const canTransact = !!wallet.provider && !!wallet.signer;
  const chainId = result.nft.chainId;
  const wrongNetwork = canTransact && !wallet.onChain(chainId);
  const order = result.order && result.order.status === "open" ? result.order : null;

  const openDialog = (next: Dialog) => {
    setNotice(null);
    setDialog(next);
  };
  const finished = (text: string) => {
    setDialog(null);
    setNotice(text);
    asset.reload();
  };

  return (
    <div className="shop-land-page shop-land-manage">
      <Link className="shop-land-back" to={tokenHref}>
        ← Back to {result.nft.name}
      </Link>

      {notice ? (
        <div className="shop-land-note" role="status">
          <p>{notice}</p>
        </div>
      ) : null}

      {!canTransact ? (
        <div className="shop-land-note shop-land-note--warn" role="note">
          <p>
            Listing, renting out, claiming and transferring LAND are {chainLabel(chainId)} transactions: they need a web3 wallet for {account} with some{" "}
            {gasTokenSymbol(chainId)} to pay the network fees.
          </p>
          {wallet.host?.onConnectWallet ? (
            <Button size="sm" onClick={() => wallet.host?.onConnectWallet?.()}>
              Connect the wallet
            </Button>
          ) : null}
        </div>
      ) : wrongNetwork ? (
        <div className="shop-land-note shop-land-note--warn" role="status">
          <p>
            Your wallet is on {wallet.chainId ? chainLabel(wallet.chainId) : "another network"}. Managing this {assetWord.toLowerCase()} runs on {chainLabel(chainId)}.
          </p>
          <Button size="sm" variant="secondary" disabled={wallet.pendingChainId !== null} onClick={() => void wallet.switchTo(chainId)}>
            {wallet.pendingChainId !== null ? "Confirm in your wallet…" : `Switch to ${chainLabel(chainId)}`}
          </Button>
        </div>
      ) : null}

      <div className="shop-land-detail">
        <div className="shop-land-detail__aside">
          <LandMapPreview result={result} tiles={tiles.data?.tiles ?? null} />
          <section className="shop-land-box" aria-labelledby="shop-land-builder">
            <h3 id="shop-land-builder">Scenes and permissions</h3>
            <p className="shop-land-page__sub">
              Deploy scenes, grant operator permissions and point a NAME to this LAND from the Creator Hub.
            </p>
            <div className="shop-land-actions">
              <Button as="a" href={BUILDER_LAND_HREF} aria-disabled={locked || undefined} onClick={(e) => locked && e.preventDefault()}>
                Open in Builder
              </Button>
            </div>
            {locked ? <p className="shop-land-page__sub">This action is locked until the rent has finished and the {assetWord.toLowerCase()} has been claimed back.</p> : null}
          </section>
          <LandHighlights result={result} tiles={tiles.data?.tiles ?? null} />
          <LandDetails result={result} rental={rental} />
        </div>

        <div className="shop-land-detail__main">
          <EstateUpgradeWarning result={result} listingCreatedAt={order?.createdAt} isOwnListing />
          <section className="shop-land-box" aria-labelledby="shop-land-title">
            <div className="shop-land-box__head">
              <h1 id="shop-land-title">{result.nft.name}</h1>
              {locked ? (
                <Button variant="secondary" disabled title={`Locked until the rent has finished and the ${assetWord.toLowerCase()} has been claimed back.`}>
                  Transfer
                </Button>
              ) : (
                <Button as="a" variant="secondary" href={`${tokenHref}/transfer`}>
                  Transfer
                </Button>
              )}
            </div>
            {d.kind === "estate" || d.kind === "parcel" ? <p className="shop-land-page__sub">{d.description || "No description"}</p> : null}
          </section>

          <section className="shop-land-box" aria-labelledby="shop-land-sell">
            <div className="shop-land-box__head">
              <h2 id="shop-land-sell">{order ? "Selling" : "Sell"}</h2>
              {locked ? (
                <Button size="sm" disabled title={`Locked until the rent has finished and the ${assetWord.toLowerCase()} has been claimed back.`}>
                  {order ? "Edit listing" : "List for sale"}
                </Button>
              ) : (
                <Button as="a" size="sm" href={`${tokenHref}/sell`}>
                  {order ? "Edit listing" : "List for sale"}
                </Button>
              )}
            </div>
            {order ? (
              <div className="shop-land-box__cols">
                <div className="shop-land-box__col">
                  <span>Price</span>
                  <Mana wei={order.price} />
                </div>
                <div className="shop-land-box__col">
                  <span>Expiration date</span>
                  <span>{dateText(epochMs(order.expiresAt))}</span>
                </div>
              </div>
            ) : null}
          </section>

          <RentBox
            rental={rental}
            assetWord={assetWord}
            claimable={claimable}
            now={now}
            tokenHref={tokenHref}
            disabled={!canTransact || wrongNetwork}
            onList={() => openDialog({ kind: "list", again: listAgain })}
            onClaim={() => openDialog({ kind: "claim" })}
          />

          {d.kind === "estate" ? <EstateParcels parcels={d.parcels} total={d.size} /> : null}
          <RentalHistory contractAddress={contractAddress} tokenId={tokenId} client={props.client} />
        </div>
      </div>

      {dialog && wallet.provider && wallet.signer ? (
        dialog.kind === "list" ? (
          <RentalListingModal
            asset={{ chainId, contractAddress, tokenId, openRentalId: result.nft.openRentalId, name: result.nft.name }}
            rental={isRentalOpen(rental) || dialog.again ? rental : null}
            listForRentAgain={dialog.again}
            account={wallet.signer}
            provider={wallet.provider}
            client={props.client}
            portal={props.portal}
            timing={props.timing}
            onClose={() => setDialog(null)}
            onRemove={() => setDialog({ kind: "remove" })}
            onDone={() => finished(`${result.nft.name} is listed for rent.`)}
          />
        ) : rental ? (
          <RentalTxModal
            kind={dialog.kind}
            asset={{ chainId, contractAddress, tokenId, openRentalId: result.nft.openRentalId, category: d.kind === "estate" ? "estate" : "parcel" }}
            rental={rental}
            account={wallet.signer}
            provider={wallet.provider}
            client={props.client}
            portal={props.portal}
            timing={props.timing}
            onClose={() => setDialog(null)}
            onDone={() => finished(dialog.kind === "claim" ? `${result.nft.name} is back in your wallet.` : "The rental listing was removed.")}
          />
        ) : null
      ) : null}
    </div>
  );
}

function RentBox({
  rental,
  assetWord,
  claimable,
  now,
  tokenHref,
  disabled,
  onList,
  onClaim,
}: {
  rental: RentalListing | null;
  assetWord: string;
  claimable: boolean;
  now: number;
  tokenHref: string;
  disabled: boolean;
  onList(): void;
  onClaim(): void;
}) {
  const open = isRentalOpen(rental);
  const executed = isRentalExecuted(rental);
  const ended = rental ? hasRentalEnded(rental, now) : false;
  const end = rental ? rentalEndDate(rental) : null;
  const title = rental && !isRentalCancelled(rental) && !(claimable && !open) ? "Renting" : "Rent";
  const periods = rental ? sortPeriods(rental.periods).map((p) => p.minDays).join(" / ") : "";
  return (
    <section className="shop-land-box" aria-labelledby="shop-land-rent">
      <div className="shop-land-box__head">
        <h2 id="shop-land-rent">{title}</h2>
        <div className="shop-land-actions">
          {open ? (
            <Button as="a" size="sm" variant="ghost" href={tokenHref}>
              View listing
            </Button>
          ) : null}
          {!rental || (isRentalCancelled(rental) && !claimable) ? (
            <Button size="sm" onClick={onList} disabled={disabled}>
              List for rent
            </Button>
          ) : open ? (
            <Button size="sm" variant="secondary" onClick={onList} disabled={disabled}>
              Edit listing
            </Button>
          ) : null}
        </div>
      </div>
      {rental && executed && !ended && end ? (
        <p>
          Rented until {dateText(end.getTime())} by {rental.tenant}
        </p>
      ) : null}
      {rental && claimable ? (
        <div className="shop-land-note" role="status">
          <p>
            {ended && rental.tenant
              ? `Rented period over! You can claim your ${assetWord.toLowerCase()} back from ${rental.tenant}.`
              : `You have an active listing, but the ${assetWord.toLowerCase()} is still unclaimed. Claim it back to sell or transfer it.`}
          </p>
          <Button size="sm" onClick={onClaim} disabled={disabled}>
            Claim {assetWord}
          </Button>
          {!open ? (
            <Button size="sm" variant="secondary" onClick={onList} disabled={disabled}>
              List for rent again
            </Button>
          ) : null}
        </div>
      ) : null}
      {rental && (open || (!claimable && !isRentalCancelled(rental))) ? (
        <div className="shop-land-box__cols">
          <div className="shop-land-box__col">
            <span>Price</span>
            <Mana wei={maxPriceOfPeriods(rental)} suffix="/day" />
          </div>
          {open ? (
            <>
              <div className="shop-land-box__col">
                <span>Listing expiration date</span>
                <span>{dateText(rental.expiration)}</span>
              </div>
              <div className="shop-land-box__col">
                <span>Rent periods</span>
                <span>{periods} days</span>
              </div>
            </>
          ) : executed && rental.startedAt && end ? (
            <>
              <div className="shop-land-box__col">
                <span>Rent start date</span>
                <span>
                  {relativeText(rental.startedAt, now)} ({dateText(rental.startedAt)})
                </span>
              </div>
              <div className="shop-land-box__col">
                <span>Rent end date</span>
                <span>
                  {relativeText(end.getTime(), now)} ({dateText(end.getTime())})
                </span>
              </div>
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function NonLandManage({ result, tokenHref }: { result: NftResult; tokenHref: string }) {
  const d = landData(result);
  return (
    <div className="shop-land-page">
      <Link className="shop-land-back" to={tokenHref}>
        ← Back to {result.nft.name}
      </Link>
      <section className="shop-land-box">
        <h1>{result.nft.name}</h1>
        {d.kind === "name" ? (
          <>
            <p className="shop-land-page__sub">
              A NAME's World, its scenes and the LAND it points to are managed in the Creator Hub. You can sell or transfer it from the Shop.
            </p>
            <div className="shop-land-actions">
              <Button as="a" href={BUILDER_NAMES_HREF}>
                Manage your NAMEs
              </Button>
              <Button as="a" variant="secondary" href={`${tokenHref}/sell`}>
                Sell
              </Button>
              <Button as="a" variant="secondary" href={`${tokenHref}/transfer`}>
                Transfer
              </Button>
            </div>
          </>
        ) : (
          <div className="shop-land-actions">
            <Button as="a" href={`${tokenHref}/sell`}>
              Sell
            </Button>
            <Button as="a" variant="secondary" href={`${tokenHref}/transfer`}>
              Transfer
            </Button>
          </div>
        )}
      </section>
    </div>
  );
}
