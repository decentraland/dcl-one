// TODO(figma-ds): replace with the profile header, until the Figma-generated one from library/ui/components.
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import { Icon } from './icons'
import styles from './ProfileHeader.module.css'

type ProfileHeaderProps = {
  address: string
  name: string
  /** The profile's name colour, also the avatar fallback background and the verified mark. */
  nameColor: string
  /** A claimed NAME shows the verified mark; an unclaimed one the `#1234` suffix. */
  hasClaimedName: boolean
  avatarUrl?: string
  /** Buttons on the right: the owner's calls to action, or a member's. */
  actions?: ReactNode
}

/** `0x1234…abcd`, as the profile site truncates addresses. */
function truncateAddress(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address
}

function CopyAddress({ address, className }: { address: string; className?: string }) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])
  const copy = () => {
    void navigator.clipboard?.writeText(address).then(() => {
      setCopied(true)
      clearTimeout(timer.current)
      timer.current = setTimeout(() => setCopied(false), 2000)
    })
  }
  return (
    <span className={styles.copyWrap}>
      <button
        type="button"
        className={className ?? styles.copyButton}
        aria-label="Copy address"
        onClick={copy}
      >
        <Icon name="ContentCopy" />
      </button>
      {copied ? (
        <span className={styles.copied} role="status">
          Address copied!
        </span>
      ) : null}
    </span>
  )
}

/** Avatar, name and address on the left, actions on the right; stacked on narrow screens. */
function ProfileHeader({
  address,
  name,
  nameColor,
  hasClaimedName,
  avatarUrl,
  actions
}: ProfileHeaderProps) {
  // A wallet with no profile yet is named by its short address and has no number.
  const anonymous = name.toLowerCase() === address.toLowerCase()
  return (
    <header className={styles.header}>
      <div className={styles.identity}>
        <ProfileAvatar imageUrl={avatarUrl} name={name} color={nameColor} />
        <div className={styles.nameBlock}>
          <div className={styles.nameRow}>
            <h1 className={styles.name} style={{ color: nameColor }}>
              {anonymous ? truncateAddress(address) : name}
            </h1>
            {hasClaimedName ? (
              <span
                className={styles.verified}
                style={{ backgroundColor: nameColor }}
                title="Verified"
                aria-label="Verified"
              >
                ✓
              </span>
            ) : anonymous ? null : (
              <span className={styles.discriminator}>#{address.slice(-4)}</span>
            )}
          </div>
          <div className={styles.addressRow}>
            <span className={styles.walletIcon}>
              <Icon name="AccountBalanceWalletOutlined" />
            </span>
            <p className={styles.addressText}>{truncateAddress(address)}</p>
            <CopyAddress address={address} />
          </div>
        </div>
      </div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </header>
  )
}

export { CopyAddress, ProfileHeader, truncateAddress }
export type { ProfileHeaderProps }
