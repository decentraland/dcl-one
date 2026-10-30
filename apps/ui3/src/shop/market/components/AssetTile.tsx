// from: shop@94584a5c:app/src/components/AssetCard (the owned-token card of My Assets);
//       catalyrst-v2 ui3/src/marketplace/components/AssetPreviewTile.tsx (the tile markup)
// One owned token in a grid: thumbnail, name, rarity, and its open listing. A token's listing is a
// resale, so the price shown is the MANA the order signed (or "USD-pegged, paid in MANA"), never credits.

import type { ReactNode } from "react";
import { Link } from "react-router";

import ThumbnailImage from "../../../components/ThumbnailImage";
import { shopPath } from "../../routes";
import type { MyAsset } from "../lib/api";
import { ManaAmount, manaNetworkOf } from "./kit";

export function listingLabel(asset: Pick<MyAsset, "listing" | "chainId">): ReactNode {
  const price = asset.listing?.price;
  if (!price) return "On sale";
  if (price.kind === "mana") return <ManaAmount wei={price.wei} network={manaNetworkOf(asset.chainId)} />;
  const cents = price.usdWei / 10n ** 16n;
  return (
    <span className="mk-usd" title="Paid in MANA at the rate when it sells">
      ${(Number(cents) / 100).toFixed(2)} in MANA
    </span>
  );
}

export default function AssetTile({ asset, actions }: { asset: MyAsset; actions?: ReactNode }) {
  const href = shopPath("tokenDetail", { contractAddress: asset.contractAddress, tokenId: asset.tokenId });
  return (
    <article className="mk-tile" data-testid="shop-asset-tile" aria-label={asset.name}>
      <Link to={href} className="mk-tile__media" data-rarity={asset.rarity ?? undefined}>
        {asset.image ? <ThumbnailImage src={asset.image} fallbackSrc={asset.image} alt="" loading="lazy" decoding="async" /> : <span className="mk-tile__glyph" aria-hidden="true">{asset.name.slice(0, 1)}</span>}
      </Link>
      <div className="mk-tile__body">
        <Link to={href} className="mk-tile__name u-truncate">
          {asset.name}
          {asset.issuedId ? <span className="mk-tile__issued"> #{asset.issuedId}</span> : null}
        </Link>
        <div className="mk-tile__meta">
          {asset.rarity ? <span className="mk-badge" data-rarity={asset.rarity}>{asset.rarity}</span> : <span className="mk-badge">{asset.category}</span>}
          {asset.isOnSale ? (
            <span className="mk-tile__price" data-testid="shop-asset-price">
              {listingLabel(asset)}
            </span>
          ) : asset.openRentalId ? (
            <span className="mk-tile__muted">Listed for rent</span>
          ) : (
            <span className="mk-tile__muted">Not for sale</span>
          )}
        </div>
        {actions ? <div className="mk-tile__actions">{actions}</div> : null}
      </div>
    </article>
  );
}
