// from: marketplace@f3956826:webapp/src/components/AssetPage/{ProximityHighlights/ProximityHighlights,
// EstateDetail/EstateDetail,EstateDetail/ParcelCoordinates/ParcelCoordinates,RentalHistory/RentalHistory,
// RentalHistory/utils}.tsx, webapp/src/components/ManageAssetPage/{Highlights/Highlights,Map/Map}.tsx,
// webapp/src/components/DetailsBox/DetailsBox.tsx, webapp/src/components/EstateUpgradeWarning/EstateUpgradeWarning.tsx
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { useMemo, useState } from "react";

import Button from "../../atoms/Button";
import type { NftResult } from "@catalyst-api/market/NftResult";
import { shortAddress } from "../model/address";
import type { ShopClient } from "../api/shop";
import { nodeMapImage } from "../../data/thumbnail";
import { landData, landParcels } from "./api";
import { isEstateListingAffectedByUpgrade } from "./estate";
import { dateText, epochMs, Mana } from "./format";
import LandMap from "./LandMap";
import { distanceText, estateProximity, parcelProximity, proximityIndex, PROXIMITY_TITLE, type ProximityKind } from "./proximity";
import { fetchRentalListings, isRentalExecuted, rentalEndDate, type RentalListing } from "./rentals";
import { selectionCenter, type Tiles } from "./tiles";
import { useResource } from "./useResource";
import "./land.css";

export function LandMapPreview({ result, tiles, label }: { result: NftResult; tiles: Tiles | null; label?: string }) {
  const parcels = useMemo(() => landParcels(result), [result]);
  const d = landData(result);
  if (!tiles) {
    return result.nft.image ? (
      <div className="shop-land-card__thumb">
        <img src={nodeMapImage(result.nft.image)} alt={`Map of ${result.nft.name}`} />
      </div>
    ) : null;
  }
  return (
    <LandMap
      className="shop-land-detail__map"
      tiles={tiles}
      selection={parcels}
      isEstate={d.kind === "estate"}
      center={parcels.length ? selectionCenter(parcels) : undefined}
      initialZoom={8}
      withPopup={false}
      label={label ?? `Map of ${result.nft.name}`}
    />
  );
}

export function LandHighlights({ result, tiles }: { result: NftResult; tiles: Tiles | null }) {
  const proximity = useMemo(() => {
    if (!tiles) return undefined;
    const index = proximityIndex(tiles);
    const d = landData(result);
    if (d.kind === "parcel") return parcelProximity({ x: d.x, y: d.y }, index, tiles[`${d.x},${d.y}`]);
    if (d.kind === "estate") return estateProximity(d.parcels, index);
    return undefined;
  }, [result, tiles]);
  if (!proximity) return null;
  const kinds = (Object.keys(proximity) as ProximityKind[]).filter((k) => proximity[k] !== undefined);
  if (!kinds.length) return null;
  return (
    <section className="shop-land-box" aria-labelledby="shop-land-highlights">
      <h3 id="shop-land-highlights">Highlights</h3>
      <ul className="shop-land-highlights">
        {kinds.map((k) => (
          <li key={k}>
            {PROXIMITY_TITLE[k]}
            <span>{distanceText(proximity[k]!)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function LandDetails({ result, rental }: { result: NftResult; rental: RentalListing | null }) {
  const d = landData(result);
  const end = rental && isRentalExecuted(rental) ? rentalEndDate(rental) : null;
  return (
    <section className="shop-land-box" aria-labelledby="shop-land-details">
      <h3 id="shop-land-details">Item details</h3>
      <dl className="shop-land-dl">
        <div>
          <dt>Type</dt>
          <dd>{d.kind === "estate" ? `Estate (${d.size} ${d.size === 1 ? "parcel" : "parcels"})` : d.kind === "parcel" ? `Parcel ${d.x},${d.y}` : result.nft.category}</dd>
        </div>
        <div>
          <dt>Network</dt>
          <dd>{result.nft.network === "MATIC" ? "Polygon" : "Ethereum"}</dd>
        </div>
        <div>
          <dt>Owner</dt>
          <dd title={result.nft.owner}>{shortAddress(result.nft.owner)}</dd>
        </div>
        {result.order ? (
          <div>
            <dt>Expires at</dt>
            <dd>{dateText(epochMs(result.order.expiresAt))}</dd>
          </div>
        ) : null}
        {end ? (
          <div>
            <dt>Rental expiration</dt>
            <dd>{dateText(end.getTime())}</dd>
          </div>
        ) : null}
      </dl>
      {d.kind === "parcel" && d.estate ? <p className="shop-land-page__sub">Part of {d.estate.name}</p> : null}
    </section>
  );
}

export function EstateParcels({ parcels, total }: { parcels: { x: number; y: number }[]; total: number }) {
  const [all, setAll] = useState(false);
  if (total === 0) {
    return (
      <div className="shop-land-note shop-land-note--warn" role="status">
        <p>This Estate has been dissolved.</p>
      </div>
    );
  }
  const shown = all ? parcels : parcels.slice(0, 16);
  return (
    <section className="shop-land-box" aria-labelledby="shop-land-parcels">
      <h3 id="shop-land-parcels">Parcels</h3>
      <ul className="shop-land-highlights">
        {shown.map((p) => (
          <li key={`${p.x},${p.y}`}>
            {p.x},{p.y}
          </li>
        ))}
      </ul>
      {parcels.length > 16 ? (
        <Button size="sm" variant="ghost" onClick={() => setAll((a) => !a)} aria-expanded={all}>
          {all ? "Show less" : `Show ${parcels.length - 16} more`}
        </Button>
      ) : null}
    </section>
  );
}

export function EstateUpgradeWarning({ result, listingCreatedAt, isOwnListing }: { result: NftResult; listingCreatedAt: number | null | undefined; isOwnListing: boolean }) {
  const d = landData(result);
  if (d.kind !== "estate" || !isEstateListingAffectedByUpgrade(d.size, listingCreatedAt)) return null;
  return (
    <div className="shop-land-note shop-land-note--warn" role="alert">
      <p>{isOwnListing ? "This listing is no longer available. You need to recreate it." : "This listing is no longer available. The seller needs to recreate it."}</p>
    </div>
  );
}

const HISTORY_PAGE = 12;

export function RentalHistory({ contractAddress, tokenId, client }: { contractAddress: string; tokenId: string; client?: ShopClient }) {
  const [page, setPage] = useState(1);
  const history = useResource(
    (signal) => fetchRentalListings({ contractAddresses: [contractAddress], tokenId, status: ["executed", "claimed"], limit: HISTORY_PAGE, page: (page - 1) * HISTORY_PAGE }, { client, signal }),
    [contractAddress, tokenId, page, client],
  );
  const rows = history.data?.results ?? [];
  if (history.status !== "ready" || rows.length === 0) return null;
  const pages = Math.max(1, Math.ceil((history.data?.total ?? 0) / HISTORY_PAGE));
  return (
    <section className="shop-land-box" aria-labelledby="shop-land-rentals">
      <h3 id="shop-land-rentals">Latest rentals</h3>
      <table className="shop-land-table">
        <thead>
          <tr>
            <th scope="col">Lessor</th>
            <th scope="col">Tenant</th>
            <th scope="col">Started at</th>
            <th scope="col">Price</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const period = r.periods.find((p) => p.maxDays === r.rentedDays) ?? r.periods[0];
            return (
              <tr key={r.id}>
                <td>{r.lessor ? shortAddress(r.lessor) : "-"}</td>
                <td>{r.tenant ? shortAddress(r.tenant) : "-"}</td>
                <td>{r.startedAt ? dateText(r.startedAt) : "-"}</td>
                <td>
                  {period ? (
                    <>
                      {r.rentedDays ?? period.maxDays} days at <Mana wei={period.pricePerDay} suffix="/day" />
                    </>
                  ) : (
                    "-"
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {pages > 1 ? (
        <div className="shop-land-actions">
          <Button size="sm" variant="secondary" disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>
            Previous
          </Button>
          <Button size="sm" variant="secondary" disabled={page >= pages} onClick={() => setPage((p) => p + 1)}>
            Next
          </Button>
        </div>
      ) : null}
    </section>
  );
}
