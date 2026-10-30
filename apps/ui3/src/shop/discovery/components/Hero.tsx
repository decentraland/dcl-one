// from: shop@2d3bf62:app/src/components/CollectionHero/CollectionHero.tsx;
//       shop@94584a5c:app/src/components/CollectionCreatorCard/CollectionCreatorCard.tsx, app/src/components/CreatorBadge/CreatorBadge.tsx

import { Link } from "react-router";

import { shortAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { useProfile } from "../hooks";
import { t } from "../intl";
import "./hero.css";

export function CollectionHero({ name, cover }: { name: string; cover?: string | null }) {
  return (
    <section className="shop-hero" data-testid="collection-hero" aria-label={name}>
      <div className="shop-hero__cover" style={cover ? { backgroundImage: `url("${cover}")` } : undefined} aria-hidden />
      <h1 className="shop-hero__title" data-testid="collection-hero-title">
        {name}
      </h1>
    </section>
  );
}

export function CreatorBadge({ address, link = true, prefix = false }: { address: string; link?: boolean; prefix?: boolean }) {
  const profile = useProfile(address);
  const name = profile?.name || shortAddress(address);
  const body = (
    <>
      <span className="shop-badge__avatar">{profile?.picture ? <img src={profile.picture} alt="" /> : null}</span>
      <span className="shop-badge__name">{prefix ? t("collection.by", { name }) : name}</span>
    </>
  );
  return link ? (
    <Link className="shop-badge" to={`${SHOP_BASE}/items/creator/${address}`} data-testid="creator-badge">
      {body}
    </Link>
  ) : (
    <span className="shop-badge" data-testid="creator-badge">
      {body}
    </span>
  );
}
