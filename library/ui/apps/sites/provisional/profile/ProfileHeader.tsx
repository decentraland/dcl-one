// TODO(figma-ds): replace with the Figma-generated profile header from library/ui/components.
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { GeneralCheck, GeneralCopy, GeneralWallet } from '../../../../foundations/icons'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import styles from './ProfileHeader.module.css'

type ProfileHeaderProps = {
  address: string
  name: string
  /** The profile's name color, also the avatar fallback background. */
  nameColor: string
  /** A claimed NAME shows the verified mark; an unclaimed one the `#1234` suffix. */
  hasClaimedName: boolean
  avatarUrl?: string
  /** Buttons on the right: the owner's calls to action, or a member's. */
  actions?: ReactNode
}

/** `0x1234…abcd`, as the source truncates addresses. */
function truncateAddress(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address
}

function CopyAddress({ address }: { address: string }) {
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
      <button type="button" className={styles.copy} aria-label="Copy address" onClick={copy}>
        <GeneralCopy size={16} aria-hidden="true" />
      </button>
      {copied ? (
        <span className={styles.copied} role="status">
          <GeneralCheck size={14} aria-hidden="true" /> Address copied!
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
  const style = { '--name-color': nameColor } as CSSProperties
  return (
    <header className={styles.root} style={style}>
      <div className={styles.identity}>
        <ProfileAvatar
          imageUrl={avatarUrl}
          name={name}
          color={nameColor}
          className={styles.avatar}
        />
        <div className={styles.nameBlock}>
          <div className={styles.nameRow}>
            <h1 className={styles.name}>{name}</h1>
            {hasClaimedName ? (
              <span className={styles.verified} title="Verified" aria-label="Verified">
                ✓
              </span>
            ) : (
              <span className={styles.discriminator}>#{address.slice(-4)}</span>
            )}
          </div>
          <div className={styles.addressRow}>
            <GeneralWallet size={18} aria-hidden="true" />
            <span className={styles.address}>{truncateAddress(address)}</span>
            <CopyAddress address={address} />
          </div>
        </div>
      </div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </header>
  )
}

export { ProfileHeader, truncateAddress }
export type { ProfileHeaderProps }
