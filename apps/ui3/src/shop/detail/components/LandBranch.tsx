// from: marketplace@3bf3662f:webapp/src/components/AssetPage/{AssetPage.tsx (renderNFTAssetPage),
// BaseDetail/BaseDetail.tsx, ParcelDetail/ParcelDetail.tsx, EstateDetail/EstateDetail.tsx,
// EstateDetail/ParcelCoordinates/ParcelCoordinates.tsx, OnBack/OnBack.tsx, Title/Title.tsx, JumpIn/JumpIn.tsx,
// Description/Description.tsx, Owner/Owner.tsx, ProximityHighlights/ProximityHighlights.tsx,
// Highlight/Highlight.tsx, SaleRentActionBox/SaleRentActionBox.tsx}, webapp/src/components/Coordinate/Coordinate.tsx,
// webapp/src/components/DetailsBox/{DetailsBox.tsx,Info/Info.tsx,DetailsRow/{Type,Expiration}/*.tsx},
// webapp/src/components/BidButton/BidButton.tsx, AssetPage/{BidList/BidList.tsx,
// TransactionHistory/{TransactionHistory,utils}.tsx, RentalHistory/{RentalHistory,utils}.tsx,
// SaleRentActionBox/PeriodsDropdown/PeriodsDropdown.tsx}, components/Bid/Bid.tsx,
// components/Table/{TableContainer,TableContent}/*.tsx; icons vendored from webapp/src/images into landbranch-assets/.
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useContext, useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router";
import { QueryClientContext } from "@tanstack/react-query";

import ManaMark from "../../../atoms/ManaMark";
import type { NftResult } from "@catalyst-api/market/NftResult";
import { landTokenId } from "../../../data/catalyst/landListing";
import { EstateUpgradeWarning } from "../../land/LandSections";
import { fetchNftResult, indexOutage, landData, landParcels, type LandNftData } from "../../land/api";
import { landCategoryOf, landNetwork } from "../../land/contracts";
import { isEstateListingAffectedByUpgrade } from "../../land/estate";
import { dateText, epochMs, manaText } from "../../land/format";
import { distanceText, estateProximity, parcelProximity, proximityIndex, type Proximity } from "../../land/proximity";
import { canBeClaimed, fetchRentalListings, hasRentalEnded, isOwnedBy, isRentalExecuted, isRentalOpen, maxPriceOfPeriods, rentalEndDate, type RentalListing } from "../../land/rentals";
import { fetchBidsByAsset } from "../../market/lib/api";
import { shopClient } from "../../api/shop";
import type { Bid, Sale } from "../../market/lib/api";
import { fetchTiles, type Tiles } from "../../land/tiles";
import { sameAddress, shortAddress } from "../../model/address";
import { parseManaWei, type ManaWei } from "../../model/money";
import { SHOP_BASE, shopPath } from "../../routes";
import { useProfile, useSeo } from "../../discovery/hooks";
import { useShopQuery } from "../../discovery/query";
import { useManaRate } from "../../credits/hooks";
import { manaWeiToUsdCents } from "../../checkout/lib/currency";
import { useShopWallet } from "../../wallet/context";
import LegacyOrderBuy from "./LegacyOrderBuy";
import LandAtlas from "./LandAtlas";
import LandTradeBuy from "./LandTradeBuy";
import backIcon from "./landbranch-assets/onBack.png";
import pinIcon from "./landbranch-assets/pin-land.svg";
import jumpIcon from "./landbranch-assets/jump_in.svg";
import offerIcon from "./landbranch-assets/make_offer.svg";
import plazaIcon from "./landbranch-assets/plaza.svg";
import roadIcon from "./landbranch-assets/road.svg";
import districtIcon from "./landbranch-assets/district.svg";
import "./landbranch.css";

export function isLandToken(contractAddress: string | null | undefined, tokenId: string | null | undefined): boolean {
  return !!tokenId && !!contractAddress && !!landCategoryOf(contractAddress);
}

export function jumpInHref(x: number, y: number): string {
  return `/play/?position=${x},${y}`;
}

const DESCRIPTION_MAX = 148;
const PARCELS_ONE_ROW = 11;

export default function LandBranch({ contractAddress: rawContract, tokenId, viewer }: { contractAddress: string; tokenId: string; viewer?: string | null }) {
  const contractAddress = rawContract.toLowerCase();
  const wallet = useShopWallet();
  const account = viewer === undefined ? wallet.account : viewer;
  const nft = useShopQuery(["land-token", contractAddress, tokenId], (signal) => fetchNftResult(contractAddress, tokenId, { signal }), { staleMs: 0 });
  const listing = useShopQuery(
    nft.data && !nft.data.order ? ["land-token-order", contractAddress, tokenId, nft.data.nft.owner] : null,
    (signal) =>
      shopClient()
        .market.orders<{ data?: NftResult["order"][] }>({ contractAddress, tokenId, status: "open" }, { signal })
        .then((r) => r.data ?? []),
    { staleMs: 0 },
  );
  const result = useMemo(() => withOpenOrder(nft.data ?? null, listing.data ?? null), [nft.data, listing.data]);
  const tiles = useShopQuery(["atlas-tiles"], (signal) => fetchTiles({ signal }), { staleMs: 5 * 60_000 });
  const category = landCategoryOf(contractAddress)?.category ?? "parcel";
  useSeo({ title: result?.nft.name || (nft.isLoading ? null : category === "estate" ? "Estate" : "Parcel") });

  const backTo = `${SHOP_BASE}/lands?section=${category === "estate" ? "estates" : "parcels"}`;

  if (!result) {
    const outage = nft.error ? indexOutage(nft.error) : null;
    return (
      <div className="shop-landtoken" data-testid="land-branch" data-state={nft.isLoading ? "loading" : nft.error ? "error" : "missing"} aria-busy={nft.isLoading || undefined}>
        <BackLink to={backTo} />
        <div className="shop-landtoken__header shop-landtoken__header--empty" />
        {nft.isLoading ? null : (
          <div className="shop-landtoken__container">
            <div className="shop-landtoken__notice" role={nft.error ? "alert" : "status"}>
              <p>{nft.error ? (outage?.message ?? "This LAND could not be loaded.") : "This LAND was not found."}</p>
              {nft.error ? (
                <button type="button" className="shop-landtoken__link" onClick={() => nft.refetch()}>
                  TRY AGAIN
                </button>
              ) : (
                <Link className="shop-landtoken__link" to={backTo}>
                  BROWSE LAND
                </Link>
              )}
            </div>
          </div>
        )}
      </div>
    );
  }
  return (
    <LandTokenView
      result={result}
      tiles={tiles.data?.tiles ?? null}
      account={account ?? null}
      backTo={backTo}
      onSignIn={wallet.host?.onSignIn}
      onChanged={() => {
        nft.refetch();
        listing.refetch();
      }}
    />
  );
}

export function withOpenOrder(result: NftResult | null, orders: readonly (NftResult["order"] | null)[] | null, now: number = Date.now()): NftResult | null {
  if (!result || result.order || !orders?.length) return result;
  const open = orders.find((o) => !!o && o.status === "open" && sameAddress(o.owner, result.nft.owner) && epochMs(o.expiresAt) > now && o.tokenId === result.nft.tokenId);
  return open ? { ...result, order: open } : result;
}

export function BackLink({ to }: { to: string }) {
  return (
    <div className="shop-landtoken__top">
      <Link className="shop-landtoken__back" to={to}>
        <img src={backIcon} alt="" aria-hidden />
        Back
      </Link>
    </div>
  );
}

type ViewProps = { result: NftResult; tiles: Tiles | null; account: string | null; backTo: string; onSignIn?: () => void; onChanged(): void };

function LandTokenView({ result, tiles, account, backTo, onSignIn, onChanged }: ViewProps) {
  const land = landData(result);
  const { nft, order, rental } = result;
  const parcels = useMemo(() => landParcels(result), [result]);
  const isEstate = land.kind === "estate";
  const dissolved = land.kind === "estate" && land.size === 0;
  const description = land.kind === "parcel" || land.kind === "estate" ? land.description : null;
  const first = parcels[0];

  return (
    <div className={`shop-landtoken${isEstate ? " shop-landtoken--estate" : ""}`} data-testid="land-branch" data-kind={land.kind}>
      <BackLink to={backTo} />
      <div className={`shop-landtoken__header${dissolved ? " shop-landtoken__header--dissolved" : ""}`}>
        {tiles ? <LandAtlas className="shop-landtoken__map" tiles={tiles} selection={parcels} isEstate={isEstate} chainId={nft.chainId} label={`Map of ${nft.name}`} /> : null}
        {dissolved ? (
          <div className="shop-landtoken__dissolved">
            <div>This Estate has been dissolved</div>
          </div>
        ) : null}
      </div>
      <div className="shop-landtoken__container">
        <div className="shop-landtoken__info">
          <div className="shop-landtoken__left">
            <div>
              <h1 className="shop-landtoken__title">{nft.name}</h1>
              <div className="shop-landtoken__badges">
                {land.kind === "parcel" ? <Coordinate x={land.x} y={land.y} /> : null}
                {land.kind === "estate" ? <span className="shop-landtoken__badge">{land.size.toLocaleString("en-US")} LAND</span> : null}
                {first && !dissolved ? <JumpIn x={first.x} y={first.y} /> : null}
              </div>
            </div>
            <Description text={description} />
            <Owner address={nft.owner} />
            <LandTokenHighlights result={result} tiles={tiles} />
          </div>
          <div className="shop-landtoken__right">
            <div className="shop-landtoken__action-box">
              <SaleRentBox result={result} land={land} account={account} onSignIn={onSignIn} onChanged={onChanged} />
            </div>
            <DetailsBox result={result} land={land} rental={rental} order={order} />
          </div>
        </div>
        <div className="shop-landtoken__below">
          {land.kind === "estate" && land.size > 0 ? <ParcelCoordinates parcels={land.parcels} total={land.size} /> : null}
          <BidList contractAddress={nft.contractAddress} tokenId={nft.tokenId} account={account} />
          <TransactionHistory contractAddress={nft.contractAddress} tokenId={nft.tokenId} />
          <RentalHistory contractAddress={nft.contractAddress} tokenId={nft.tokenId} />
        </div>
      </div>
    </div>
  );
}

function Coordinate({ x, y }: { x: number; y: number }) {
  return (
    <span className="shop-landtoken__badge">
      <img className="shop-landtoken__pin" src={pinIcon} alt="" aria-hidden />
      {`${x},${y}`}
    </span>
  );
}

function JumpIn({ x, y }: { x: number; y: number }) {
  return (
    <a className="shop-landtoken__badge shop-landtoken__badge--jump" href={jumpInHref(x, y)} target="_blank" rel="noopener noreferrer" data-testid="land-jump-in">
      Jump in
      <img src={jumpIcon} alt="" aria-hidden />
    </a>
  );
}

function Description({ text }: { text: string | null }) {
  const [more, setMore] = useState(false);
  const long = !!text && text.length > DESCRIPTION_MAX;
  return (
    <div className="shop-landtoken__description">
      <h2 className="shop-landtoken__subheader">Description</h2>
      <div className="shop-landtoken__description-text">{text ? (long && !more ? `${text.substring(0, DESCRIPTION_MAX)}...` : text) : "No description"}</div>
      {long ? (
        <button type="button" className="shop-landtoken__link" onClick={() => setMore((m) => !m)} aria-expanded={more}>
          {more ? "READ LESS" : "READ MORE"}
        </button>
      ) : null}
    </div>
  );
}

function Avatar({ address, size }: { address: string; size: number }) {
  const profile = useProfile(address);
  return profile?.picture ? (
    <img className="shop-landtoken__avatar" src={profile.picture} alt="" width={size} height={size} />
  ) : (
    <span className="shop-landtoken__avatar shop-landtoken__avatar--blank" style={{ width: size, height: size }} aria-hidden />
  );
}

function ProfileName({ address }: { address: string }) {
  const profile = useProfile(address);
  return <>{profile?.name || shortAddress(address)}</>;
}

function Owner({ address }: { address: string }) {
  return (
    <div className="shop-landtoken__owner">
      <h2 className="shop-landtoken__stat-title">Owner</h2>
      <Link className="shop-landtoken__profile" to={`${SHOP_BASE}/accounts/${address}`} title={address} data-testid="land-owner">
        <Avatar address={address} size={48} />
        <span className="shop-landtoken__profile-name">
          <ProfileName address={address} />
        </span>
      </Link>
    </div>
  );
}

const HIGHLIGHT: { kind: keyof Proximity; name: string; icon: string }[] = [
  { kind: "plaza", name: "Plaza", icon: plazaIcon },
  { kind: "road", name: "Road", icon: roadIcon },
  { kind: "district", name: "District", icon: districtIcon },
];

export function LandTokenHighlights({ result, tiles }: { result: NftResult; tiles: Tiles | null }) {
  const proximity = useMemo<Proximity | undefined>(() => {
    if (!tiles) return undefined;
    const land = landData(result);
    const index = proximityIndex(tiles);
    if (land.kind === "parcel") return parcelProximity({ x: land.x, y: land.y }, index, tiles[`${land.x},${land.y}`]);
    if (land.kind === "estate") return estateProximity(land.parcels, index);
    return undefined;
  }, [tiles, result]);
  return proximity ? <Highlights proximity={proximity} /> : null;
}

export function LandTokenDetails({ result }: { result: NftResult }) {
  return <DetailsBox result={result} land={landData(result)} rental={result.rental} order={result.order} />;
}

export function LandTokenMap({ result, tiles }: { result: NftResult; tiles: Tiles | null }) {
  const parcels = useMemo(() => landParcels(result), [result]);
  const land = landData(result);
  return (
    <div className="shop-landtoken__header">
      {tiles ? (
        <LandAtlas
          className="shop-landtoken__map"
          tiles={tiles}
          selection={parcels}
          isEstate={land.kind === "estate"}
          chainId={result.nft.chainId}
          label={`Map of ${result.nft.name}`}
        />
      ) : null}
    </div>
  );
}

function Highlights({ proximity }: { proximity: Proximity }) {
  const shown = HIGHLIGHT.filter((h) => proximity[h.kind] !== undefined);
  if (!shown.length) return null;
  return (
    <div className="shop-landtoken__highlights" data-testid="land-highlights">
      <h2 className="shop-landtoken__subheader">Highlights</h2>
      <div className="shop-landtoken__highlight-row">
        {shown.map((h) => (
          <div key={h.kind} className="shop-landtoken__highlight" data-kind={h.kind}>
            <span className="shop-landtoken__highlight-icon">
              <img src={h.icon} alt="" aria-hidden />
            </span>
            <span className="shop-landtoken__highlight-text">
              <span className="shop-landtoken__highlight-name">{h.name}</span>
              <span className="shop-landtoken__highlight-desc">{distanceText(proximity[h.kind]!)}</span>
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function FiatPrice({ wei, suffix }: { wei: string; suffix?: string }) {
  return useContext(QueryClientContext) ? <FiatPriceRead wei={wei} suffix={suffix} /> : null;
}

function FiatPriceRead({ wei, suffix }: { wei: string; suffix?: string }) {
  const rate = useManaRate();
  if (!rate.data) return null;
  let cents: number;
  try {
    cents = manaWeiToUsdCents(wei, rate.data);
  } catch {
    return null;
  }
  if (!Number.isFinite(cents)) return null;
  return (
    <span className="shop-landtoken__fiat">
      ({(cents / 100).toLocaleString("en-US", { style: "currency", currency: "USD" })}
      {suffix ?? ""})
    </span>
  );
}

type SaleRentBoxProps = { result: NftResult; land: LandNftData; account: string | null; onSignIn?: () => void; onChanged(): void };

function SaleRentBox({ result, land, account, onSignIn, onChanged }: SaleRentBoxProps) {
  const { nft, order, rental } = result;
  const asset = { chainId: nft.chainId, contractAddress: nft.contractAddress, tokenId: nft.tokenId, owner: nft.owner };
  const rentalOpen = isRentalOpen(rental);
  const isOwner = isOwnedBy(asset, account, rental);
  const isTenant = !!rental?.tenant && !!account && sameAddress(rental.tenant, account);
  const brokenListing = isEstateListingAffectedByUpgrade(land.kind === "estate" ? land.size : 0, order?.createdAt);
  const maxPrice = rental ? maxPriceOfPeriods(rental) : null;
  const [view, setView] = useState<"sale" | "rent">(!!order || !rentalOpen ? "sale" : "rent");
  const [buying, setBuying] = useState(false);
  const [period, setPeriod] = useState<number | null>(null);
  const partOfEstate = land.kind === "parcel" && !!land.estate;
  const executed = isRentalExecuted(rental);
  const endDate = executed && rental ? rentalEndDate(rental) : null;
  const ended = executed && rental ? hasRentalEnded(rental) : false;
  const params = { contractAddress: nft.contractAddress, tokenId: nft.tokenId };
  const assetType = land.kind === "estate" ? "estate" : "parcel";
  const priceWei = order ? parseManaWei(order.price) : null;
  const buyable = !!order && !!priceWei && !brokenListing;

  return (
    <div className="shop-landtoken__sale" data-testid="land-sale-box">
      {order ? <EstateUpgradeWarning result={result} listingCreatedAt={order.createdAt} isOwnListing={isOwner} /> : null}
      {rentalOpen && maxPrice ? (
        <div className="shop-landtoken__views" role="tablist">
          <button type="button" role="tab" aria-selected={view === "sale"} disabled={view === "sale"} className="shop-landtoken__view" onClick={() => setView("sale")}>
            Sale
          </button>
          <button type="button" role="tab" aria-selected={view === "rent"} disabled={view === "rent"} className="shop-landtoken__view" onClick={() => setView("rent")}>
            Rent
          </button>
        </div>
      ) : null}
      <div className="shop-landtoken__actions">
        {view === "rent" && rentalOpen && maxPrice ? (
          <>
            <div className="shop-landtoken__price">
              <div className="shop-landtoken__price-title">Price</div>
              <div className="shop-landtoken__price-value">
                <ManaMark size={18} network="ethereum" />
                <span className="shop-landtoken__price-mana">{manaText(maxPrice)}</span>
                <span className="shop-landtoken__per-day">/day</span>
                <FiatPrice wei={maxPrice} suffix="/day" />
              </div>
            </div>
            {rental ? <PeriodPicker periods={rental.periods} value={period} onChange={setPeriod} /> : null}
            {!isOwner ? (
              <Link
                className="shop-landtoken__primary"
                to={`${shopPath("tokenRent", params)}${period !== null ? `?period=${period}` : ""}`}
                data-testid="land-rent"
                aria-disabled={partOfEstate || period === null || undefined}
                title={partOfEstate ? "This parcel is part of an Estate and can't be rented on its own." : undefined}
                onClick={partOfEstate || period === null ? (e) => e.preventDefault() : undefined}
              >
                Rent
              </Link>
            ) : null}
          </>
        ) : (
          <>
            {order && priceWei ? (
              <div className="shop-landtoken__price">
                <div className="shop-landtoken__price-title">Price</div>
                <div className="shop-landtoken__price-value" data-testid="land-price">
                  <ManaMark size={18} network={order.network === "MATIC" ? "polygon" : "ethereum"} />
                  <span className="shop-landtoken__price-mana">{manaText(order.price)}</span>
                  <FiatPrice wei={order.price} />
                </div>
              </div>
            ) : isOwner && rental?.tenant && endDate && !ended ? (
              <div className="shop-landtoken__upper">
                This {assetType} is rented by {shortAddress(rental.tenant)} until <strong>{dateText(endDate.getTime())}</strong>.
              </div>
            ) : isTenant && endDate && !ended ? (
              <div className="shop-landtoken__upper">
                You are renting this {assetType} until <strong>{dateText(endDate.getTime())}</strong>.
              </div>
            ) : (isTenant || isOwner) && endDate && ended ? (
              <div className="shop-landtoken__upper">
                The rent of this {assetType} ended on <strong>{dateText(endDate.getTime())}</strong>.
              </div>
            ) : (
              <div className="shop-landtoken__upper" data-testid="land-not-for-sale">
                Currently not listed for sale
              </div>
            )}
            {isOwner && rental && account && canBeClaimed(account, rental, asset) ? (
              <div className="shop-landtoken__upper-desc">You can claim this {assetType} back from the manage page.</div>
            ) : null}
            {!isOwner ? (
              <>
                {isTenant && !ended ? (
                  <a className="shop-landtoken__primary shop-landtoken__primary--builder" href="/create/land">
                    Manage in Builder
                  </a>
                ) : null}
                <div className="shop-landtoken__sale-buttons">
                  {buyable ? (
                    <button type="button" className="shop-landtoken__primary" onClick={() => (account ? setBuying(true) : onSignIn?.())} data-testid="land-buy">
                      <ManaMark size={16} network="ethereum" />
                      <span>Buy with MANA</span>
                    </button>
                  ) : null}
                  <Link
                    className="shop-landtoken__bid"
                    to={shopPath("tokenBid", params)}
                    data-testid="make-offer"
                    aria-disabled={partOfEstate || undefined}
                    title={partOfEstate ? "This parcel is part of an Estate and can't receive offers on its own." : undefined}
                    onClick={partOfEstate ? (e) => e.preventDefault() : undefined}
                  >
                    <img src={offerIcon} alt="" aria-hidden />
                    Make offer
                  </Link>
                </div>
              </>
            ) : null}
          </>
        )}
        {isOwner ? (
          <Link className="shop-landtoken__manage" to={shopPath("tokenManage", params)} data-testid="land-manage">
            Manage
          </Link>
        ) : null}
      </div>
      {executed && endDate && !ended && !isTenant && !isOwner ? (
        <div className="shop-landtoken__message">
          This LAND is rented until <span className="shop-landtoken__rented-until">{dateText(endDate.getTime())}</span>
        </div>
      ) : null}
      {buying && order && priceWei && order.tradeId ? (
        <LandTradeBuy name={nft.name} tradeId={order.tradeId} quoted={priceWei as ManaWei} onClose={() => setBuying(false)} onDone={() => onChanged()} />
      ) : null}
      {buying && order && priceWei && !order.tradeId ? (
        <LegacyOrderBuy
          name={nft.name}
          contractAddress={nft.contractAddress}
          tokenId={nft.tokenId}
          order={{ id: order.id, marketplaceAddress: order.marketplaceAddress, price: order.price, chainId: order.chainId }}
          quoted={priceWei as ManaWei}
          onClose={() => setBuying(false)}
          onDone={() => onChanged()}
        />
      ) : null}
    </div>
  );
}

function Info({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="shop-landtoken__row">
      <div className="shop-landtoken__row-title">{title}</div>
      <div className="shop-landtoken__row-content">{children}</div>
    </div>
  );
}

function estateContract(chainId: number): string {
  try {
    return landNetwork(chainId).estate;
  } catch {
    return landNetwork().estate;
  }
}

function DetailsBox({ result, land, rental, order }: { result: NftResult; land: LandNftData; rental: RentalListing | null; order: NftResult["order"] }) {
  const { nft } = result;
  const activeRental = !!rental && (rental.status === "open" || rental.status === "executed");
  const owner = activeRental && rental?.lessor ? rental.lessor : nft.owner;
  return (
    <div className="shop-landtoken__box" data-testid="land-details">
      <div className="shop-landtoken__box-header">Item details</div>
      <div className="shop-landtoken__box-content">
        <Info title="Type">
          {land.kind === "estate" ? (
            `Estate (${land.size})`
          ) : land.kind === "parcel" && land.estate ? (
            <>
              Parcel (part of <Link to={`${SHOP_BASE}/token/${estateContract(nft.chainId)}/${land.estate.tokenId}`}>{land.estate.name || "Estate"}</Link>)
            </>
          ) : (
            "Parcel"
          )}
        </Info>
        <Info title="Network">{nft.network}</Info>
        {order ? <Info title="Order expiration">{relativeTime(epochMs(order.expiresAt))}</Info> : null}
        {rental ? <Info title="Rental expiration">{relativeTime(epochMs(rental.expiration))}</Info> : null}
        {owner ? (
          <Info title="Owner">
            <Link className="shop-landtoken__inline-profile" to={`${SHOP_BASE}/accounts/${owner}`} title={owner}>
              <Avatar address={owner} size={20} />
              <ProfileName address={owner} />
            </Link>
          </Info>
        ) : null}
      </div>
    </div>
  );
}

function ParcelCoordinates({ parcels: unsorted, total }: { parcels: { x: number; y: number }[]; total: number }) {
  const [open, setOpen] = useState(false);
  const parcels = useMemo(() => [...unsorted].sort((a, b) => a.y - b.y || a.x - b.x), [unsorted]);
  const landContract = useMemo(() => landNetwork().land, []);
  return (
    <div className="shop-landtoken__parcels" data-testid="land-parcels">
      <h2 className="shop-landtoken__subheader">Parcels</h2>
      <div className={`shop-landtoken__coordinates${open ? "" : " shop-landtoken__coordinates--collapsed"}`}>
        {parcels.map((p) => (
          <Link key={`${p.x},${p.y}`} to={`${SHOP_BASE}/token/${landContract}/${landTokenId(p.x, p.y)}`}>
            <Coordinate x={p.x} y={p.y} />
          </Link>
        ))}
        {parcels.length < total ? (
          <span className="shop-landtoken__more-text">
            and <strong>{total - parcels.length}</strong> more
          </span>
        ) : null}
      </div>
      {parcels.length > PARCELS_ONE_ROW ? (
        <div className="shop-landtoken__show-more">
          <button type="button" className="shop-landtoken__link" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
            {open ? "SHOW LESS" : "SHOW MORE"}
          </button>
        </div>
      ) : null}
    </div>
  );
}

const SHORT_DAY = new Intl.DateTimeFormat("en-US", { month: "short", day: "2-digit" });

function PeriodPicker({ periods, value, onChange }: { periods: RentalListing["periods"]; value: number | null; onChange(i: number | null): void }) {
  const start = Date.now();
  return (
    <>
      <div className="shop-landtoken__period-title">Period</div>
      <select
        className="shop-landtoken__period"
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
        aria-label="Select period"
        data-testid="land-rent-period"
      >
        <option value="" disabled>
          Select period
        </option>
        {periods.map((p, i) => {
          const total = (BigInt(p.pricePerDay) * BigInt(p.maxDays)).toString();
          const until = `${SHORT_DAY.format(new Date(start))} - ${SHORT_DAY.format(new Date(start + p.maxDays * 86_400_000))}`;
          return (
            <option key={i} value={i}>
              {`${p.maxDays} days (${until}) · ${manaText(total)} MANA`}
            </option>
          );
        })}
      </select>
    </>
  );
}

function LinkedProfile({ address }: { address: string }) {
  return (
    <Link className="shop-landtoken__inline-profile" to={`${SHOP_BASE}/accounts/${address}`} title={address}>
      <Avatar address={address} size={24} />
      <ProfileName address={address} />
    </Link>
  );
}

function InlineMana({ wei, network }: { wei: string; network?: string }) {
  return (
    <span className="shop-landtoken__inline-mana">
      <ManaMark size={14} network={network === "MATIC" || network === "polygon" ? "polygon" : "ethereum"} />
      {manaText(wei)}
    </span>
  );
}

const WEEK_MS = 7 * 86_400_000;
const LONG_DAY = new Intl.DateTimeFormat("en-US", { month: "long", day: "numeric", year: "numeric" });

function pppText(ms: number): string {
  const parts = LONG_DAY.formatToParts(new Date(ms));
  const day = Number(parts.find((p) => p.type === "day")?.value ?? 0);
  const month = parts.find((p) => p.type === "month")?.value ?? "";
  const year = parts.find((p) => p.type === "year")?.value ?? "";
  const ord = day % 100 >= 11 && day % 100 <= 13 ? "th" : (["th", "st", "nd", "rd"][day % 10] ?? "th");
  return `${month} ${day}${ord}, ${year}`;
}

function eventDate(ms: number): string {
  return Date.now() - ms > WEEK_MS ? pppText(ms) : relativeTime(ms);
}

type Column = { title: string; wide?: boolean };

function HistoryTable({ title, columns, rows, page, total, perPage, onPage, testId }: { title: string; columns: Column[]; rows: ReactNode[][]; page: number; total: number; perPage: number; onPage(p: number): void; testId: string }) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  return (
    <div className="shop-landtoken__table" data-testid={testId}>
      <div className="shop-landtoken__table-tabs">
        <div className="shop-landtoken__table-title">{title}</div>
      </div>
      <table className="shop-landtoken__table-grid">
        <tbody>
          <tr>
            {columns.map((c) => (
              <th key={c.title} scope="col" data-wide={c.wide || undefined}>
                <span>{c.title}</span>
              </th>
            ))}
          </tr>
          {rows.map((cells, r) => (
            <tr key={r}>
              {cells.map((cell, i) => (
                <td key={columns[i]!.title} data-wide={columns[i]!.wide || undefined} style={{ width: `${100 / columns.length}%` }}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {pages > 1 ? (
        <div className="shop-landtoken__pagination">
          <span>{`Showing ${(page - 1) * perPage + 1}-${Math.min(page * perPage, total)}  of ${total}`}</span>
          <nav className="shop-landtoken__pages" aria-label={`${title} pages`}>
            {Array.from({ length: pages }, (_, i) => i + 1)
              .filter((p) => p === 1 || p === pages || Math.abs(p - page) <= 1)
              .map((p, i, shown) => (
                <span key={p}>
                  {i > 0 && p - shown[i - 1]! > 1 ? <span className="shop-landtoken__page-gap">…</span> : null}
                  <button type="button" className="shop-landtoken__page" aria-current={p === page ? "page" : undefined} onClick={() => onPage(p)}>
                    {p}
                  </button>
                </span>
              ))}
          </nav>
        </div>
      ) : null}
    </div>
  );
}

const SALES_PER_PAGE = 5;
const SALE_TYPE: Record<string, string> = { order: "Listing", bid: "Bid", mint: "Mint" };

function TransactionHistory({ contractAddress, tokenId }: { contractAddress: string; tokenId: string }) {
  const [page, setPage] = useState(1);
  const sales = useShopQuery(
    ["land-sales", contractAddress, tokenId, page],
    (signal) => shopClient().market.sales<{ data?: Sale[]; total?: number }>({ contractAddress, tokenId, first: SALES_PER_PAGE, skip: (page - 1) * SALES_PER_PAGE }, { signal }),
    { keepPrevious: true },
  );
  const rows = sales.data?.data ?? [];
  if (!rows.length) return null;
  return (
    <div className="shop-landtoken__history">
      <HistoryTable
        title="Latest Sales"
        testId="land-sales"
        columns={[{ title: "From", wide: true }, { title: "To", wide: true }, { title: "Type" }, { title: "When" }, { title: "Price" }]}
        rows={rows.map((sale) => [
          <LinkedProfile key="from" address={sale.seller} />,
          <LinkedProfile key="to" address={sale.buyer} />,
          SALE_TYPE[sale.type] ?? sale.type,
          eventDate(epochMs(sale.timestamp)),
          <InlineMana key="price" wei={sale.price} network={sale.network} />,
        ])}
        page={page}
        total={sales.data?.total ?? rows.length}
        perPage={SALES_PER_PAGE}
        onPage={setPage}
      />
    </div>
  );
}

const RENTALS_PER_PAGE = 12;

function RentalHistory({ contractAddress, tokenId }: { contractAddress: string; tokenId: string }) {
  const [page, setPage] = useState(1);
  const history = useShopQuery(
    ["land-rental-history", contractAddress, tokenId, page],
    (signal) =>
      fetchRentalListings(
        { contractAddresses: [contractAddress], tokenId, status: ["executed", "claimed"], limit: RENTALS_PER_PAGE, page: (page - 1) * RENTALS_PER_PAGE },
        { signal },
      ),
    { keepPrevious: true },
  );
  const rows = history.data?.results ?? [];
  if (!rows.length) return null;
  return (
    <HistoryTable
      title="Latest rentals"
      testId="land-rentals"
      columns={[{ title: "Lessor" }, { title: "Tenant" }, { title: "Started at" }, { title: "Days" }, { title: "Price" }]}
      rows={rows.map((r) => {
        const chosen = r.periods.find((p) => p.maxDays === r.rentedDays) ?? r.periods[0];
        return [
          r.lessor ? <LinkedProfile key="lessor" address={r.lessor} /> : "-",
          r.tenant ? <LinkedProfile key="tenant" address={r.tenant} /> : "-",
          eventDate(r.startedAt ?? 0),
          `${r.rentedDays ?? 0} days`,
          chosen ? <InlineMana key="price" wei={chosen.pricePerDay} network={r.network} /> : "-",
        ];
      })}
      page={page}
      total={history.data?.total ?? rows.length}
      perPage={RENTALS_PER_PAGE}
      onPage={setPage}
    />
  );
}

function BidList({ contractAddress, tokenId, account }: { contractAddress: string; tokenId: string; account: string | null }) {
  const bids = useShopQuery(["land-bids", contractAddress, tokenId], () => fetchBidsByAsset({ contractAddress, tokenId }, { status: "open" }));
  const rows: Bid[] = bids.data?.results ?? [];
  if (!rows.length) return null;
  const params = { contractAddress, tokenId };
  return (
    <div className="shop-landtoken__bids" data-testid="land-bids">
      <h2 className="shop-landtoken__subheader">Bids</h2>
      <div className="shop-landtoken__bid-list">
        {rows.map((bid) => {
          const isBidder = !!account && sameAddress(account, bid.bidder);
          const isSeller = !!account && sameAddress(account, bid.seller);
          return (
            <div key={bid.id} className="shop-landtoken__bid-row">
              <div className="shop-landtoken__stat">
                <div className="shop-landtoken__stat-sub">From</div>
                <LinkedProfile address={bid.bidder} />
              </div>
              <div className="shop-landtoken__stat">
                <div className="shop-landtoken__stat-sub">Created</div>
                <div className="shop-landtoken__stat-value">{relativeTime(epochMs(bid.createdAt))}</div>
              </div>
              <div className="shop-landtoken__stat">
                <div className="shop-landtoken__stat-sub">Price</div>
                <div className="shop-landtoken__stat-value">
                  <InlineMana wei={bid.price} network={bid.network} />
                </div>
              </div>
              <div className="shop-landtoken__stat">
                <div className="shop-landtoken__stat-sub">Time Left</div>
                <div className="shop-landtoken__stat-value">{relativeTime(epochMs(bid.expiresAt)).replace(/^in /, "")}</div>
              </div>
              {isBidder || isSeller ? (
                <div className="shop-landtoken__bid-actions">
                  {isBidder && !bid.tradeId ? (
                    <Link className="shop-landtoken__small shop-landtoken__small--primary" to={shopPath("tokenBid", params)}>
                      Update
                    </Link>
                  ) : null}
                  {isBidder || isSeller ? (
                    <Link className="shop-landtoken__small" to={`${shopPath("tokenBids", params)}?bid=${encodeURIComponent(bid.id)}`}>
                      {isSeller ? "Accept" : "Cancel"}
                    </Link>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function relativeTime(ms: number, now: number = Date.now()): string {
  const future = ms >= now;
  const minutes = Math.round(Math.abs(ms - now) / 60_000);
  let words: string;
  if (minutes < 1) words = "less than a minute";
  else if (minutes < 45) words = `${minutes} minute${minutes === 1 ? "" : "s"}`;
  else if (minutes < 90) words = "about 1 hour";
  else if (minutes < 1440) words = `about ${Math.round(minutes / 60)} hours`;
  else if (minutes < 2520) words = "1 day";
  else if (minutes < 43200) words = `${Math.round(minutes / 1440)} days`;
  else if (minutes < 86400) {
    const m = Math.round(minutes / 43200);
    words = `about ${m} month${m === 1 ? "" : "s"}`;
  } else {
    const months = Math.floor(minutes / 43200);
    if (months < 12) words = `${months} months`;
    else {
      const years = Math.floor(months / 12);
      const rest = months % 12;
      const y = `${years} year${years === 1 ? "" : "s"}`;
      words = rest < 3 ? `about ${y}` : rest < 9 ? `over ${y}` : `almost ${years + 1} years`;
    }
  }
  return future ? `in ${words}` : `${words} ago`;
}
