// from: shop@2d3bf62:app/src/components/AssetCard/AssetCard.tsx, app/src/components/AssetCard/AssetCard.styles.ts
//       (the favorites grid's card: artwork on the rarity wash, name, "By {creator}", price, rarity and category chips, heart)

import type { ReactNode } from "react";
import { Link } from "react-router";

import { Coin } from "../../../atoms/icons";
import ThumbnailImage from "../../../components/ThumbnailImage";
import { publicThumbnail, publicThumbnailSet } from "../../../data/thumbnail";
import Icon from "../../discovery/components/Icon";
import { CreditsAmount } from "../../discovery/components/PriceTag";
import { useCreatorName } from "../../discovery/hooks";
import { t } from "../../discovery/intl";
import { categoryIcon } from "../../discovery/lib/itemIcons";
import { rarityColor, rarityDescription, rarityLabel, rarityMedia } from "../../discovery/lib/rarity";
import { shopPath } from "../../routes";
import type { FavoriteItem } from "../lib/catalog";
import "../../discovery/components/assetcard.css";
import "../../discovery/components/pricetag.css";
import "../pages/shopfavorites.css";

export function CreditsPrice({ credits }: { credits: number }) {
  return (
    <span className="mk-mana">
      <Coin size={14} />
      <span>{credits.toLocaleString("en-US")}</span>
      <span className="u-sr-only">{credits === 1 ? " credit" : " credits"}</span>
    </span>
  );
}

export function itemPrice(item: Pick<FavoriteItem, "isOnSale" | "priceCredits">) {
  if (!item.isOnSale)
    return (
      <span className="shop-price" data-kind="not-for-sale">
        {t("assetCard.notForSale")}
      </span>
    );
  if (item.priceCredits === null)
    return (
      <span className="shop-price" data-kind="credits-unavailable" data-testid="shop-credits-unavailable">
        <Icon name="warning-fill" size={14} />
        <span>Credits price unavailable</span>
      </span>
    );
  return (
    <span className="shop-price" data-kind="credits">
      <CreditsAmount credits={item.priceCredits} />
    </span>
  );
}

export default function ItemCard({ item, saved, onToggleSave, actions }: { item: FavoriteItem; saved?: boolean; onToggleSave?: () => void; actions?: ReactNode }) {
  const href = shopPath("itemDetail", { contractAddress: item.contractAddress, itemId: item.itemId });
  const author = useCreatorName(item.creator);
  const catIco = categoryIcon({ category: item.category });
  return (
    <div className="shop-favcard">
      <article className="shop-card" data-testid="shop-item-card" aria-label={item.name}>
        <Link className="shop-card__link" to={href} aria-label={item.name} />
        {onToggleSave ? (
          <button
            type="button"
            className="shop-card__fav"
            data-on={saved || undefined}
            aria-pressed={!!saved}
            aria-label={saved ? `Remove ${item.name} from favorites` : `Save ${item.name}`}
            onClick={onToggleSave}
          >
            <Icon name={saved ? "heart-solid" : "heart"} size={16} />
          </button>
        ) : null}
        <div className="shop-card__media" style={{ backgroundImage: rarityMedia(item.rarity) }}>
          {item.thumbnail ? (
            <ThumbnailImage className="shop-card__img" src={publicThumbnail(item.thumbnail, 320)} srcSet={publicThumbnailSet(item.thumbnail, [320, 640])}
              sizes="(max-width: 640px) 45vw, 240px" fallbackSrc={item.thumbnail} alt="" loading="lazy" decoding="async" />
          ) : null}
        </div>
        <div className="shop-card__body">
          <div className="shop-card__top">
            <div className="shop-card__desc">
              <span className="shop-card__name" title={item.name}>
                {item.name}
              </span>
              <span className="shop-card__sub">{author ? t("search.byCreator", { name: author }) : " "}</span>
            </div>
            <span data-testid="shop-item-price">{itemPrice(item)}</span>
          </div>
          <div className="shop-card__action">
            <div className="shop-card__chips" data-chips>
              <span className="shop-card__chip" data-variant="rarity" style={{ background: rarityColor(item.rarity) }} title={rarityDescription(item.rarity)}>
                {rarityLabel(item.rarity)}
              </span>
              {catIco ? (
                <span className="shop-card__chip" data-variant="icon">
                  <Icon name={catIco} size={14.6} />
                </span>
              ) : null}
            </div>
          </div>
        </div>
      </article>
      {actions ? <div className="shop-favcard__actions">{actions}</div> : null}
    </div>
  );
}
