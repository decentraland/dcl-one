import type { CSSProperties, ReactNode } from "react";

import { ProfilePic } from "@dcl/one-ui";
import { GeneralVerified, GeneralWallet } from "@dcl/one-ui/foundations/icons";

import { CopyAddress } from "../components/CopyAddress";

import styles from "./ProfileHeader.module.css";

type ProfileHeaderProps = {
  address: string;
  /** The address as the app shortens it for display (`0x1234…abcd`); the copy button copies `address`. */
  addressLabel: string;
  name: string;
  nameColor: string;
  /** A claimed NAME shows the verified mark; an unclaimed one the `#1234` suffix. */
  hasClaimedName: boolean;
  avatarUrl?: string;
  /** Buttons on the right: the owner's calls to action, or a member's. */
  actions?: ReactNode;
};

/** Avatar, name and address on the left, actions on the right; stacked on narrow screens. */
export function ProfileHeader({
  address,
  addressLabel,
  name,
  nameColor,
  hasClaimedName,
  avatarUrl,
  actions,
}: ProfileHeaderProps) {
  return (
    <header className={styles.root} style={{ "--name-color": nameColor } as CSSProperties}>
      <div className={styles.identity}>
        <ProfilePic className={styles.avatar} name={name} src={avatarUrl} size={48} nameColor={nameColor} />
        <div className={styles.nameBlock}>
          <div className={styles.nameRow}>
            <h1 className={styles.name}>{name}</h1>
            {hasClaimedName ? (
              <span className={styles.verified} role="img" aria-label="Verified" title="Verified">
                <GeneralVerified size={20} aria-hidden="true" />
              </span>
            ) : (
              <span className={styles.discriminator}>#{address.slice(-4)}</span>
            )}
          </div>
          <div className={styles.addressRow}>
            <GeneralWallet size={18} aria-hidden="true" />
            <span className={styles.address}>{addressLabel}</span>
            <CopyAddress address={address} />
          </div>
        </div>
      </div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </header>
  );
}
