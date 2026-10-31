// from: shop@2d3bf62:app/src/components/CollectionCard/CollectionCard.tsx, app/src/components/CollectionThumb/CollectionThumb.tsx (CollectionMosaic, useCollectionPreview), app/src/components/CreatorBadge/CreatorBadge.tsx
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { shortAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { fetchCollectionItems, type CollectionSummary } from "../api";
import { useProfile } from "../hooks";
import { t } from "../intl";
import { addressAvatarColor, avatarInitial } from "../lib/avatarColor";
import type { CatalogItem } from "../lib/catalogItem";
import { rarityMedia } from "../lib/rarity";
import { useShopQuery } from "../query";
import "./collectioncard.css";

const MOSAIC_COUNT = 4;

export function CollectionMosaic({ items }: { items: CatalogItem[] }) {
  const cells = items.slice(0, MOSAIC_COUNT);
  return (
    <span className="shop-colcard__mosaic" data-count={cells.length} aria-hidden>
      {cells.map((item) => (
        <span key={item.id} className="shop-colcard__cell" data-testid="coll-thumb-cell" style={{ backgroundImage: rarityMedia(item.rarity) }}>
          {item.thumbnail ? <img src={item.thumbnail} alt="" loading="lazy" /> : null}
        </span>
      ))}
    </span>
  );
}

function CreatorRow({ address }: { address: string }) {
  const navigate = useNavigate();
  const profile = useProfile(address);
  const [broken, setBroken] = useState(false);
  useEffect(() => setBroken(false), [profile?.picture]);
  const name = profile?.name ? profile.name.charAt(0).toUpperCase() + profile.name.slice(1) : shortAddress(address);
  const initial = avatarInitial(profile?.name, address);
  return (
    <button
      type="button"
      className="shop-colcard__creator"
      data-link
      data-testid="creator"
      onClick={(e) => {
        e.stopPropagation();
        navigate(`${SHOP_BASE}/items/creator/${address}`);
      }}
    >
      {profile?.picture && !broken ? (
        <img className="shop-colcard__ava" src={profile.picture} alt="" loading="lazy" onError={() => setBroken(true)} />
      ) : (
        <span className="shop-colcard__ava" data-letter style={{ backgroundColor: addressAvatarColor(address) }} aria-hidden>
          {initial}
        </span>
      )}
      <span className="shop-colcard__by" data-testid="creator-name">
        By <span className="shop-colcard__display">{name}</span>
      </span>
    </button>
  );
}

export default function CollectionCard({ collection, cover, itemCount }: { collection: CollectionSummary; cover?: string; itemCount?: number }) {
  const navigate = useNavigate();
  const { contractAddress, name, creator } = collection;
  const knownCount = itemCount ?? collection.itemCount;
  const needsItems = cover == null || knownCount == null;
  const preview = useShopQuery(needsItems ? ["collection-preview", contractAddress] : null, (signal) => fetchCollectionItems(contractAddress, { first: MOSAIC_COUNT }, { signal }), {
    staleMs: 5 * 60_000,
  });
  const mosaic = cover == null ? (preview.data?.items ?? []) : [];
  const count = knownCount ?? preview.data?.total;
  const open = () => navigate(`${SHOP_BASE}/collection/${contractAddress}`);

  return (
    <article
      className="shop-colcard"
      data-testid="collection-card"
      role="link"
      tabIndex={0}
      aria-label={`${name}: ${t("collectionCard.viewCollection")}`}
      onClick={open}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
      }}
    >
      <div className="shop-colcard__media">
        {cover ? <img className="shop-colcard__img" src={cover} alt="" loading="lazy" /> : mosaic.length > 0 ? <CollectionMosaic items={mosaic} /> : null}
      </div>
      <div className="shop-colcard__body">
        <h3 className="shop-colcard__name" title={name}>
          {name}
        </h3>
        <div className="shop-colcard__slot">
          <div className="shop-colcard__meta" data-meta data-testid="coll-card-meta">
            {creator ? <CreatorRow address={creator} /> : <span>&nbsp;</span>}
            <span className="shop-colcard__count" data-testid="coll-card-count" aria-busy={count == null || undefined}>
              {count == null ? <span className="shop-colcard__countskel" aria-hidden /> : t("collectionCard.itemCount", { count })}
            </span>
          </div>
          <button
            type="button"
            className="shop-colcard__view"
            data-view
            data-testid="coll-card-view"
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              open();
            }}
          >
            {t("collectionCard.viewCollection")}
          </button>
        </div>
      </div>
    </article>
  );
}
