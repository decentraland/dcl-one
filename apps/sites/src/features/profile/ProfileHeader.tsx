import type { CSSProperties, ReactNode } from "react";

import { GeneralVerified, GeneralWallet } from "@dcl/one-ui/foundations/icons";

import { CopyAddress } from "../components/CopyAddress";

import styles from "./ProfileHeader.module.css";
import { useBrokenImage } from "./useBrokenImage";

type ProfileHeaderProps = {
  address: string;
  /** The address as the app shortens it for display (`0x1234…abcd`); the copy button copies `address`. */
  addressLabel: string;
  name: string;
  /** The profile's name color, also the avatar fallback background. */
  nameColor: string;
  /** A claimed NAME shows the verified mark; an unclaimed one the `#1234` suffix. */
  hasClaimedName: boolean;
  avatarUrl?: string;
  /** Buttons on the right: the owner's calls to action, or a member's. */
  actions?: ReactNode;
};

function firstGrapheme(value: string): string {
  for (const ch of value) return ch;
  return "";
}

/** The round face snapshot with a soft border; the initial on the name color without one, or when it fails. */
function Avatar({ imageUrl, name }: { imageUrl?: string; name: string }) {
  const { broken, ref, onError } = useBrokenImage(imageUrl);
  return (
    <span className={styles.avatar}>
      {imageUrl && !broken ? (
        <img ref={ref} className={styles.avatarImage} src={imageUrl} alt={name} onError={onError} />
      ) : (
        <span className={styles.avatarInitial} aria-hidden="true">
          {(firstGrapheme(name) || "?").toUpperCase()}
        </span>
      )}
    </span>
  );
}

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
  const style = { "--name-color": nameColor } as CSSProperties;
  return (
    <header className={styles.root} style={style}>
      <div className={styles.identity}>
        <Avatar imageUrl={avatarUrl} name={name} />
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
