// from: shop@2d3bf62:app/src/components/CreatorHero/CreatorHero.tsx, CreatorHero.styles.ts

import type { CSSProperties } from "react";
import { Link } from "react-router";

import { shortAddress } from "../../../model/address";
import { SHOP_BASE } from "../../../routes";
import { fetchStore, LINK_TYPES, type LinkType } from "../../../seller/lib/store";
import { useShopWallet } from "../../../wallet/context";
import { useProfile } from "../../hooks";
import { useShopQuery } from "../../query";
import { getAvatarBackgroundColor, getDisplayName } from "../../lib/avatarColor";
import defaultCover from "./default-cover.jpeg?url";
import discordIcon from "./icon-discord.svg?url";
import facebookIcon from "./icon-facebook.svg?url";
import penIcon from "./icon-pen.svg?url";
import websiteIcon from "./icon-website.svg?url";
import twitterIcon from "./icon-x-twitter.svg?url";
import "./creatorhero.css";

const LINK_ICON: Record<LinkType, string> = { website: websiteIcon, twitter: twitterIcon, discord: discordIcon, facebook: facebookIcon };
const LINK_LABEL: Record<LinkType, string> = { website: "Website", twitter: "X (Twitter)", discord: "Discord", facebook: "Facebook" };

const mask = (src: string): CSSProperties => ({ WebkitMaskImage: `url("${src}")`, maskImage: `url("${src}")` });

export default function CreatorHero({ address }: { address: string }) {
  const profile = useProfile(address);
  const { data: store } = useShopQuery(["store", address.toLowerCase()], () => fetchStore(address), { staleMs: 5 * 60_000 });
  const { session } = useShopWallet();
  const self = session.kind === "web3" ? session.wallet : session.kind === "web2" ? session.account : null;
  const isOwner = !!self && self.toLowerCase() === address.toLowerCase();

  const name = profile?.name || shortAddress(address);
  const face = profile?.picture;
  const cover = store?.cover || defaultCover;
  const description = store?.description;
  const links = LINK_TYPES.filter((type) => store?.links[type]);
  const avatarBg = getAvatarBackgroundColor(getDisplayName(profile?.name?.replace(/#.*$/, ""), profile?.claimed, address));

  return (
    <section className="shop-creatorhero" data-testid="creator-hero" aria-label={`${name} storefront`}>
      <div className="shop-creatorhero__cover" aria-hidden>
        <img src={cover} alt="" loading="eager" />
        <span className="shop-creatorhero__scrim" />
      </div>

      {links.length > 0 || isOwner ? (
        <div className="shop-creatorhero__links">
          {links.map((type) => (
            <a key={type} className="shop-creatorhero__link" data-testid="creator-hero-link" href={store?.links[type] ?? ""} target="_blank" rel="noopener noreferrer" title={LINK_LABEL[type]} aria-label={LINK_LABEL[type]}>
              <span className="shop-creatorhero__ico" style={mask(LINK_ICON[type])} />
            </a>
          ))}
          {isOwner ? (
            <Link className="shop-creatorhero__link shop-creatorhero__edit" to={`${SHOP_BASE}/store-settings`} title="Edit store" aria-label="Edit store">
              <span className="shop-creatorhero__ico" style={mask(penIcon)} />
            </Link>
          ) : null}
        </div>
      ) : null}

      <div className="shop-creatorhero__body">
        {face ? (
          <img className="shop-creatorhero__ava" src={face} alt="" loading="eager" style={{ backgroundColor: avatarBg }} />
        ) : (
          <span className="shop-creatorhero__ava" aria-hidden style={{ backgroundColor: avatarBg }} />
        )}
        <div>
          <h1 className="shop-creatorhero__name" data-testid="creator-hero-name">
            {name}
          </h1>
          {description ? <p className="shop-creatorhero__desc">{description}</p> : null}
        </div>
        <div className="shop-creatorhero__actions">
          <a className="shop-creatorhero__view" data-testid="creator-hero-view" href={`/profile/${address}`} target="_blank" rel="noopener noreferrer">
            View profile
          </a>
        </div>
      </div>
    </section>
  );
}
