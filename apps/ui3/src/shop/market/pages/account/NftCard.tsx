// from: marketplace@3bf3662f:webapp/src/components/AssetCard/AssetCard.tsx (the NFT branch: title, MANA price,
//       network meta, then the Wearable/Emote/Parcel/Estate tags), webapp/src/themes/shop-parity.css (card surface)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { Link } from "react-router";

import ThumbnailImage from "../../../../components/ThumbnailImage";
import Icon from "../../../discovery/components/Icon";
import { categoryIcon, genderIcon } from "../../../discovery/lib/itemIcons";
import { rarityMedia } from "../../../discovery/lib/rarity";
import { shopPath } from "../../../routes";
import { listingLabel } from "../../components/AssetTile";
import type { AccountNft } from "./api";

const NETWORK: Record<string, string> = { MATIC: "Polygon", ETHEREUM: "Ethereum" };

export default function NftCard({ asset }: { asset: AccountNft }) {
  const href = shopPath("tokenDetail", { contractAddress: asset.contractAddress, tokenId: asset.tokenId });
  const cat = asset.category === "wearable" || asset.category === "emote" ? categoryIcon({ category: asset.category, wearableCategory: asset.wearableCategory }) : null;
  const gender = asset.category === "wearable" ? genderIcon(asset.gender) : null;
  const wash = asset.rarity ? rarityMedia(asset.rarity) : undefined;
  return (
    <Link to={href} className="acct-card" data-testid="shop-asset-tile" aria-label={asset.name}>
      <div className="acct-card__media" data-kind={asset.category} style={wash ? { backgroundImage: wash } : undefined}>
        {asset.category === "ens" ? (
          <span className="acct-card__ens">{asset.name}</span>
        ) : asset.image ? (
          <ThumbnailImage src={asset.image} fallbackSrc={asset.image} alt="" loading="lazy" decoding="async" />
        ) : null}
      </div>
      <div className="acct-card__body">
        <div className="acct-card__head">
          <span className="acct-card__name">{asset.name}</span>
          {asset.isOnSale ? (
            <span className="acct-card__price" data-testid="shop-asset-price">
              {listingLabel(asset)}
            </span>
          ) : null}
        </div>
        <div className="acct-card__meta">{NETWORK[asset.network] ?? asset.network}</div>
        <div className="acct-card__tags">
          {asset.rarity ? <span className="acct-card__rarity">{asset.rarity}</span> : null}
          {cat ? (
            <span className="acct-card__icon" title={asset.wearableCategory}>
              <Icon name={cat} size={12} />
            </span>
          ) : null}
          {gender ? (
            <span className="acct-card__icon" title={asset.gender ?? undefined}>
              <Icon name={gender} size={12} />
            </span>
          ) : null}
          {asset.isSmart ? (
            <span className="acct-card__icon" title="Smart">
              <Icon name="smart" size={12} />
            </span>
          ) : null}
          {asset.parcel ? (
            <span className="acct-card__coord">
              {asset.parcel.x},{asset.parcel.y}
            </span>
          ) : null}
          {asset.estateSize != null ? <span className="acct-card__coord">{asset.estateSize} LAND</span> : null}
          {asset.openRentalId ? <span className="acct-card__coord">Rented</span> : null}
        </div>
      </div>
    </Link>
  );
}
