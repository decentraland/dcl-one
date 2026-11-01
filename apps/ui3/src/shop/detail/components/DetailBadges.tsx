// from: shop@3a322cf9:app/src/components/CreatorBadge/CreatorBadge.tsx, app/src/components/CollectionBadge/CollectionBadge.tsx,
//       app/src/pages/ItemDetail.styles.ts (DetailCreator, DetailCollection)
import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { Link } from "react-router";

import { SHOP_BASE } from "../../routes";
import { useCreatorProfile } from "../../discovery/hooks";
import { t } from "../../discovery/intl";
import { addressAvatarColor, avatarInitial } from "../../discovery/lib/avatarColor";
import { shortAddress } from "../../model/address";
import styles from "./DetailBadges.module.css";

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** The item page's creator line: the avatar (or a coloured initial) and "By {name}", linking to the creator's storefront. */
export function DetailCreator({ address }: { address?: string }) {
  const { profile } = useCreatorProfile(address);
  const face = profile?.picture ?? null;
  // A face URL can 404 (an undeployed snapshot); fall back to the initial, and retry when the URL changes.
  const [broken, setBroken] = useState(false);
  useEffect(() => setBroken(false), [face]);
  if (!address) return null;
  const own = profile?.name?.trim();
  return (
    <Link className={styles.root} to={`${SHOP_BASE}/items/creator/${address}`} data-testid="creator">
      {face && !broken ? (
        <img className={styles.ava} data-testid="creator-ava" src={face} alt="" loading="lazy" onError={() => setBroken(true)} />
      ) : (
        <span className={styles.ava} data-letter data-testid="creator-ava-letter" style={{ "--ava-color": addressAvatarColor(address) } as CSSProperties} aria-hidden>
          {avatarInitial(own, address)}
        </span>
      )}
      <span className={styles.name} data-testid="creator-name">
        {t("search.byCreator", { name: "" })}
        <span className={styles.display} data-testid="creator-display">
          {own ? capitalize(own) : shortAddress(address)}
        </span>
      </span>
    </Link>
  );
}

/** The item's collection: a collage of up to four of its thumbnails and the name, linking to the collection. */
export function DetailCollection({ contractAddress, name, thumbnails, label }: { contractAddress?: string; name?: string; thumbnails?: string[]; label?: (name: ReactNode) => ReactNode }) {
  if (!contractAddress || !name) return null;
  const thumbs = (thumbnails ?? []).filter(Boolean).slice(0, 4);
  const row1 = thumbs.slice(0, 2);
  const row2 = thumbs.slice(2, 4);
  const display = (
    <span className={styles.display} data-testid="creator-display">
      {capitalize(name)}
    </span>
  );
  return (
    <Link className={styles.root} to={`${SHOP_BASE}/collection/${contractAddress}`} data-testid="creator" data-kind="collection">
      {thumbs.length > 0 ? (
        <span className={`${styles.ava} ${styles.collage}`} data-testid="creator-ava" aria-hidden>
          <span className={styles.row} data-full={row2.length === 0 || undefined}>
            {row1.map((src, i) => (
              <img key={i} src={src} alt="" />
            ))}
          </span>
          {row2.length > 0 ? (
            <span className={styles.row}>
              {row2.map((src, i) => (
                <img key={i} src={src} alt="" />
              ))}
            </span>
          ) : null}
        </span>
      ) : null}
      <span className={styles.name} data-testid="creator-name">
        {label ? label(display) : display}
      </span>
    </Link>
  );
}
