// from: marketplace@3bf3662f:webapp/src/components/AssetCard/{AssetCard.tsx,AssetCard.css,utils.tsx
// (formatWeiToAssetCard)}, webapp/src/components/AssetCard/{ParcelTags,EstateTags}/*.tsx (the LAND badges
// of the browse card)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
import { Link } from "react-router";

import type { NftResult } from "@catalyst-api/market/NftResult";
import { nodeMapImage } from "../../data/thumbnail";
import { landData } from "./api";
import { Mana } from "./format";
import { isRentalOpen, maxPriceOfPeriods } from "./rentals";
import "./land.css";
import "./lands.css";

export function landHref(result: NftResult): string {
  return `/shop/token/${result.nft.contractAddress.toLowerCase()}/${result.nft.tokenId}`;
}

export function landBadge(result: NftResult): string {
  const d = landData(result);
  if (d.kind === "parcel") return `${d.x},${d.y}`;
  if (d.kind === "estate") return `${d.size} ${d.size === 1 ? "parcel" : "parcels"}`;
  return "";
}

const COMPACT = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 2 });

export function cardManaText(wei: string | bigint): string {
  const n = Number(typeof wei === "bigint" ? wei : BigInt(wei)) / 1e18;
  if (n === 0) return "0";
  if (n < 0.01) return "0.01";
  return COMPACT.format(n);
}

function PinGlyph() {
  return (
    <svg width="10" height="12" viewBox="0 0 10 12" aria-hidden="true">
      <path fill="currentColor" d="M5 0a5 5 0 0 0-5 5c0 3.6 5 7 5 7s5-3.4 5-7a5 5 0 0 0-5-5Zm0 7a2 2 0 1 1 0-4 2 2 0 0 1 0 4Z" />
    </svg>
  );
}

export default function LandCard({ result, position }: { result: NftResult; position?: number }) {
  const { nft, order } = result;
  const rental = isRentalOpen(result.rental) ? result.rental : null;
  const onSale = order && order.status === "open" ? order : null;
  const d = landData(result);
  const network = nft.network === "MATIC" ? "polygon" : "ethereum";
  const rent = rental ? <Mana text={cardManaText(maxPriceOfPeriods(rental))} network={network} suffix="/day" /> : null;
  return (
    <Link className="shop-lb-card" to={landHref(result)} aria-label={`${nft.name}, ${landBadge(result)}`}>
      <div className="shop-lb-card__image">
        {nft.image ? (
          <img src={nodeMapImage(nft.image)} alt="" loading={position !== undefined && position < 6 ? "eager" : "lazy"} decoding="async"
            fetchPriority={position !== undefined && position < 3 ? "high" : undefined} />
        ) : null}
        {rental ? <span className="shop-lb-card__bubble">Listed for rent</span> : null}
      </div>
      <div className="shop-lb-card__content">
        <div className="shop-lb-card__header">
          <span className="shop-lb-card__title">{nft.name}</span>
          {onSale ? <Mana text={cardManaText(onSale.price)} network={network} /> : rent}
        </div>
        <div className="shop-lb-card__sub">
          <span className="shop-lb-card__meta">{network === "polygon" ? "Polygon" : "Ethereum"}</span>
          {onSale && rent ? <span>{rent}</span> : null}
        </div>
        <div className="shop-lb-card__tags">
          {d.kind === "parcel" ? (
            <span className="shop-lb-card__tag shop-lb-card__tag--coords">
              <PinGlyph />
              {d.x},{d.y}
            </span>
          ) : d.kind === "estate" ? (
            <span className="shop-lb-card__tag">{d.size.toLocaleString("en-US")} LAND</span>
          ) : null}
        </div>
      </div>
    </Link>
  );
}
