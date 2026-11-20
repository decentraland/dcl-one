import { useId, type ComponentType } from "react";
import { Link } from "react-router";

import { useChromeAuth } from "@ui/web/frames/chrome-auth";

import {
  GeneralGift,
  GeneralLogout,
  GeneralRightArrow,
  GeneralWallet,
  MenusNotifications,
  type IconProps,
} from "@dcl/one-ui/foundations/icons";

import { shortAddress } from "@/data/lib/catalyst/format/address";
import { CopyAddress } from "@dcl/one-ui/apps/sites/CopyAddress";
import { useBrokenImage } from "@dcl/one-ui/apps/sites/useBrokenImage";

import styles from "./AccountSidebar.module.css";
import type { AccountLinkState } from "./AccountLayout";
import { ACCOUNT_COPY } from "./copy";

export type AccountSection = "wallets" | "notifications" | "credits";

/** Security and account deletion are not built yet, so they are not listed. */
const SECTIONS: readonly { id: AccountSection; href: string; Icon: ComponentType<IconProps> }[] = [
  { id: "wallets", href: "/account/wallets", Icon: GeneralWallet },
  { id: "notifications", href: "/account/notifications", Icon: MenusNotifications },
  { id: "credits", href: "/account/credits", Icon: GeneralGift },
];

function Avatar({ imageUrl }: { imageUrl?: string }) {
  const { broken, ref, onError } = useBrokenImage(imageUrl);
  return (
    <span className={styles.avatar}>
      {imageUrl && !broken ? (
        <img ref={ref} className={styles.avatarImage} src={imageUrl} alt="" onError={onError} />
      ) : null}
    </span>
  );
}

type AccountSidebarProps = {
  address: string;
  active: AccountSection | null;
  /** Carried by each section link, so Close knows where the reader entered the settings. */
  linkState: AccountLinkState;
  className?: string;
};

/**
 * The account menu: who is signed in, the sections and log out. A rail beside the section from 1024px;
 * below it the whole `/account` screen, where each row reads as "open this section".
 */
export function AccountSidebar({ address, active, linkState, className }: AccountSidebarProps) {
  const titleId = useId();
  const auth = useChromeAuth();
  const own = auth.account.toLowerCase() === address.toLowerCase();
  const name = (own && auth.name) || shortAddress(address);

  return (
    <nav className={[styles.root, className].filter(Boolean).join(" ")} aria-labelledby={titleId}>
      <div className={styles.user}>
        <Avatar imageUrl={own ? auth.avatarUrl : undefined} />
        <div className={styles.userInfo}>
          <p className={styles.name} title={name}>
            {name}
          </p>
          <div className={styles.addressRow}>
            <GeneralWallet size={16} aria-hidden="true" />
            <span className={styles.address}>{shortAddress(address)}</span>
            <CopyAddress address={address} />
          </div>
        </div>
      </div>

      <hr className={styles.divider} />

      <h1 id={titleId} className={styles.sectionLabel}>
        {ACCOUNT_COPY.title}
      </h1>
      <ul className={styles.items}>
        {SECTIONS.map(({ id, href, Icon }) => (
          <li key={id}>
            <Link to={href} state={linkState} className={styles.item} aria-current={active === id ? "page" : undefined}>
              <Icon size={20} aria-hidden="true" />
              <span className={styles.itemLabel}>{ACCOUNT_COPY.sections[id]}</span>
              <GeneralRightArrow className={styles.chevron} size={18} aria-hidden="true" />
            </Link>
          </li>
        ))}
      </ul>

      {/* Only for the account shown: a chrome still on another account would sign that one out. */}
      {own && auth.onSignOut ? (
        <div className={styles.bottom}>
          <hr className={styles.divider} />
          <button type="button" className={styles.item} onClick={auth.onSignOut}>
            <GeneralLogout size={20} aria-hidden="true" />
            <span className={styles.itemLabel}>{ACCOUNT_COPY.logout}</span>
          </button>
        </div>
      ) : null}
    </nav>
  );
}
